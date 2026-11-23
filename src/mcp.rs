//! MCP (Model Context Protocol) server, so an AI agent can do anything a person can
//! do in Fontabulous Studio.
//!
//! Two transports, both speaking JSON-RPC 2.0:
//! * stdio — start the app with `--mcp`; the agent's client launches it and talks
//!   over stdin/stdout while the window stays open and shows every change live.
//! * HTTP — Preferences ▸ Agents turns on a local-only endpoint
//!   (`http://127.0.0.1:<port>/mcp`, Streamable HTTP with JSON replies) so an agent
//!   can attach to the window the person is already using.
//!
//! Network/stdin threads only parse and route messages. Every tool call is handed to
//! the UI thread through a channel and runs at the start of the next frame, so agent
//! actions share the exact code paths (and undo history) of menu clicks.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::app::*;
use crate::commands;
use crate::font::{Anchor, Component, Contour, Glyph};

pub const DEFAULT_PORT: u16 = 7457;
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// One tool call waiting for the UI thread.
pub struct Call {
    pub name: String,
    pub args: Value,
    pub reply: Sender<Value>,
}

struct Http {
    port: u16,
    stop: Arc<AtomicBool>,
}

pub struct Bridge {
    ctx: egui::Context,
    tx: Sender<Call>,
    rx: Receiver<Call>,
    http: Option<Http>,
    pub http_error: Option<String>,
    pub stdio: bool,
    stdio_done: Arc<AtomicBool>,
    close_sent: bool,
    pending_shots: Vec<(Sender<Value>, u32)>,
    /// Number of tool calls handled, and the last one (name, time) for the status bar.
    pub calls: u64,
    pub last_call: Option<(String, f64)>,
}

impl Bridge {
    pub fn new(ctx: &egui::Context) -> Self {
        let (tx, rx) = channel();
        Bridge { ctx: ctx.clone(), tx, rx, http: None, http_error: None, stdio: false, stdio_done: Arc::new(AtomicBool::new(false)), close_sent: false, pending_shots: Vec::new(), calls: 0, last_call: None }
    }

    pub fn http_port(&self) -> Option<u16> {
        self.http.as_ref().map(|h| h.port)
    }

    /// Serve MCP over stdin/stdout (the app was started with `--mcp`).
    pub fn start_stdio(&mut self) {
        if self.stdio {
            return;
        }
        self.stdio = true;
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let done = self.stdio_done.clone();
        std::thread::Builder::new()
            .name("mcp-stdio".into())
            .spawn(move || {
                let stdin = std::io::stdin();
                let out = Arc::new(Mutex::new(std::io::stdout()));
                for line in stdin.lock().lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let tx = tx.clone();
                    let ctx = ctx.clone();
                    let out = out.clone();
                    // Each message on its own thread so a slow call never blocks pings.
                    std::thread::spawn(move || {
                        if let Some(reply) = handle_text(&line, &tx, &ctx) {
                            let mut o = out.lock().unwrap();
                            let _ = writeln!(o, "{reply}");
                            let _ = o.flush();
                        }
                    });
                }
                // The client closed stdin: that is MCP's signal to shut down.
                done.store(true, Ordering::Relaxed);
                ctx.request_repaint();
            })
            .ok();
    }

    /// Start (or restart on a new port) the local HTTP endpoint.
    pub fn start_http(&mut self, port: u16) {
        if self.http.as_ref().map(|h| h.port) == Some(port) {
            return;
        }
        self.stop_http();
        let listener = match TcpListener::bind(("127.0.0.1", port)) {
            Ok(l) => l,
            Err(e) => {
                self.http_error = Some(format!("Could not listen on port {port}: {e}"));
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        self.http_error = None;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, ctx, stop2) = (self.tx.clone(), self.ctx.clone(), stop.clone());
        std::thread::Builder::new()
            .name("mcp-http".into())
            .spawn(move || {
                while !stop2.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let (tx, ctx) = (tx.clone(), ctx.clone());
                            std::thread::spawn(move || serve_http(stream, &tx, &ctx));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(60)),
                        Err(_) => std::thread::sleep(Duration::from_millis(200)),
                    }
                }
            })
            .ok();
        self.http = Some(Http { port, stop });
    }

    pub fn stop_http(&mut self) {
        if let Some(h) = self.http.take() {
            h.stop.store(true, Ordering::Relaxed);
        }
    }

    /// True once a stdio client has disconnected.
    pub fn stdio_finished(&self) -> bool {
        self.stdio && self.stdio_done.load(Ordering::Relaxed)
    }

    /// Close the window once the stdio client has gone (asks to save unsaved fonts).
    pub fn close_when_finished(&mut self, ctx: &egui::Context) {
        if self.stdio_finished() && !self.close_sent {
            self.close_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Keep the HTTP endpoint in step with the preferences.
    pub fn sync(&mut self, enabled: bool, port: u16) {
        if enabled {
            self.start_http(port);
        } else {
            self.stop_http();
            self.http_error = None;
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop_http();
    }
}

// ------------------------------------------------------------------------------------------
// JSON-RPC routing (runs on I/O threads)

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Handle one line/body of JSON (a message or a batch). `None` when there is nothing to send back.
fn handle_text(text: &str, tx: &Sender<Call>, ctx: &egui::Context) -> Option<Value> {
    let msg: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return Some(rpc_error(&Value::Null, -32700, &format!("Parse error: {e}"))),
    };
    if let Value::Array(items) = msg {
        let out: Vec<Value> = items.into_iter().filter_map(|m| handle_message(m, tx, ctx)).collect();
        return if out.is_empty() { None } else { Some(Value::Array(out)) };
    }
    handle_message(msg, tx, ctx)
}

fn handle_message(msg: Value, tx: &Sender<Call>, ctx: &egui::Context) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let Some(id) = id else {
        // Notifications (initialized, cancelled…) and stray responses need no answer.
        return None;
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&asked) { asked } else { PROTOCOL_VERSIONS[0] };
            rpc_result(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "fontabulous-studio", "title": "Fontabulous Studio", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => rpc_result(&id, json!({})),
        "tools/list" => rpc_result(&id, json!({"tools": tool_list()})),
        "resources/list" => rpc_result(&id, json!({"resources": []})),
        "prompts/list" => rpc_result(&id, json!({"prompts": []})),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !tool_list().iter().any(|t| t["name"] == name.as_str()) {
                return Some(rpc_error(&id, -32602, &format!("Unknown tool '{name}'")));
            }
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let (rtx, rrx) = channel();
            if tx.send(Call { name, args, reply: rtx }).is_err() {
                return Some(rpc_error(&id, -32603, "Fontabulous Studio is closing"));
            }
            ctx.request_repaint();
            match rrx.recv_timeout(Duration::from_secs(90)) {
                Ok(result) => rpc_result(&id, result),
                Err(_) => rpc_result(&id, tool_error("Timed out waiting for the app (is a dialog open that needs a person?)")),
            }
        }
        _ => rpc_error(&id, -32601, &format!("Method not found: {method}")),
    })
}

// ------------------------------------------------------------------------------------------
// Minimal HTTP/1.1 server for Streamable HTTP (JSON replies, no SSE streams)

fn serve_http(mut stream: TcpStream, tx: &Sender<Call>, ctx: &egui::Context) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Ok(clone) = stream.try_clone() else { return };
    let mut reader = BufReader::new(clone);
    loop {
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let mut len = 0usize;
        let mut origin = None;
        let mut close = false;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).unwrap_or(0) == 0 {
                return;
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
                match k.as_str() {
                    "content-length" => len = v.parse().unwrap_or(0),
                    "origin" => origin = Some(v.to_string()),
                    "connection" => close = v.eq_ignore_ascii_case("close"),
                    _ => {}
                }
            }
        }
        let mut body = vec![0u8; len.min(64 * 1024 * 1024)];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        // Block web pages from reaching the endpoint (DNS-rebinding protection).
        let origin_ok = origin.as_deref().map(|o| o == "null" || o.starts_with("http://localhost") || o.starts_with("http://127.0.0.1") || o.starts_with("http://[::1]")).unwrap_or(true);
        let route = path.split('?').next().unwrap_or("");
        let (status, payload): (&str, Option<String>) = if !origin_ok {
            ("403 Forbidden", Some(r#"{"error":"Origin not allowed"}"#.into()))
        } else if route != "/mcp" && route != "/" {
            ("404 Not Found", Some(r#"{"error":"Use /mcp"}"#.into()))
        } else {
            match method.as_str() {
                "POST" => match handle_text(&String::from_utf8_lossy(&body), tx, ctx) {
                    Some(v) => ("200 OK", Some(v.to_string())),
                    None => ("202 Accepted", None),
                },
                "DELETE" => ("200 OK", None),
                "OPTIONS" => ("204 No Content", None),
                _ => ("405 Method Not Allowed", None),
            }
        };
        let body = payload.unwrap_or_default();
        let mut head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n", body.len());
        if !body.is_empty() {
            head.push_str("Content-Type: application/json\r\n");
        }
        if status.starts_with("405") {
            head.push_str("Allow: POST, DELETE\r\n");
        }
        head.push_str(if close { "Connection: close\r\n\r\n" } else { "Connection: keep-alive\r\n\r\n" });
        if stream.write_all(head.as_bytes()).is_err() || stream.write_all(body.as_bytes()).is_err() {
            return;
        }
        let _ = stream.flush();
        if close {
            return;
        }
    }
}

// ------------------------------------------------------------------------------------------
// Tool catalogue

const INSTRUCTIONS: &str = "Fontabulous Studio is a font editor. Every menu command is available through run_command \
(see list_commands); structured tools read and change glyph outlines, metrics, kerning, font info and preferences. \
Coordinates are font units (y up, baseline 0). Points use the UFO model: each contour is a list of points whose type t is \
Move (open-contour start), Line, Curve (cubic on-curve, preceded by two Off points), QCurve (quadratic on-curve) or Off \
(control point); closed contours are cyclic. Every change is undoable (run_command edit.undo). Use screenshot to see the window.";

fn tool(name: &str, title: &str, description: &str, props: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {"type": "object", "properties": props, "required": required},
        "annotations": {"title": title, "readOnlyHint": read_only, "openWorldHint": false},
    })
}

pub fn tool_list() -> Vec<Value> {
    let contour_schema = json!({
        "type": "array",
        "description": "Contours: [{\"closed\": true, \"points\": [{\"x\": 100, \"y\": 0, \"t\": \"Line\", \"smooth\": false}, …]}]. t is Move, Line, Curve, QCurve or Off.",
        "items": {"type": "object", "properties": {
            "closed": {"type": "boolean"},
            "points": {"type": "array", "items": {"type": "object", "properties": {
                "x": {"type": "number"}, "y": {"type": "number"},
                "t": {"type": "string", "enum": ["Move", "Line", "Curve", "QCurve", "Off"]},
                "smooth": {"type": "boolean"}
            }, "required": ["x", "y", "t"]}}
        }, "required": ["points"]}
    });
    vec![
        tool("list_commands", "List commands", "List every menu command with its id, menu label, shortcut, whether it can run right now, and whether it takes a path.", json!({"filter": {"type": "string", "description": "Only ids or labels containing this text"}}), &[], true),
        tool("run_command", "Run a menu command", "Run any menu command by id (e.g. contour.reverse, font.build_glyphs, file.export_ttf, edit.undo). Commands that read or write files need \"path\". They act on the selected glyphs (font tab) or the glyph being edited (glyph tab).", json!({"id": {"type": "string"}, "path": {"type": "string", "description": "File or folder for commands that open, save, import or export"}}), &["id"], false),
        tool("get_state", "Get app state", "Open fonts, tabs, the active tab and tool, current glyph, selection, master, open dialogs and recent log lines.", json!({}), &[], true),
        tool("list_glyphs", "List glyphs", "Glyphs of the current font in order, with Unicode, width, contour/component counts and flag.", json!({"filter": {"type": "string", "description": "Name or character substring"}, "offset": {"type": "integer"}, "limit": {"type": "integer", "description": "Default 300"}}), &[], true),
        tool("get_glyph", "Get glyph", "One glyph's outline (contours, components, anchors, guides), width, sidebearings and Unicode values for a master.", json!({"name": {"type": "string"}, "master": {"type": "integer", "description": "Master index; default the current master"}}), &["name"], true),
        tool("set_glyph", "Set glyph", "Create or change a glyph. Only the fields you pass change. Undoable.", json!({
            "name": {"type": "string"},
            "master": {"type": "integer"},
            "new_name": {"type": "string"},
            "unicodes": {"type": "array", "items": {"type": "string"}, "description": "Hex values like \"0041\""},
            "width": {"type": "number"},
            "contours": contour_schema,
            "components": {"type": "array", "items": {"type": "object", "properties": {"base": {"type": "string"}, "t": {"type": "array", "items": {"type": "number"}, "description": "xx, xy, yx, yy, dx, dy (default identity)"}}, "required": ["base"]}},
            "anchors": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "x": {"type": "number"}, "y": {"type": "number"}}, "required": ["name", "x", "y"]}},
            "note": {"type": "string"}
        }), &["name"], false),
        tool("add_glyphs", "Add glyphs", "Add empty glyphs by name, character or U+XXXX.", json!({"names": {"type": "array", "items": {"type": "string"}}}), &["names"], false),
        tool("delete_glyphs", "Delete glyphs", "Remove glyphs from the font. Undoable.", json!({"names": {"type": "array", "items": {"type": "string"}}}), &["names"], false),
        tool("select_glyphs", "Select glyphs", "Select glyphs in the font window (menu commands then act on them).", json!({"names": {"type": "array", "items": {"type": "string"}}, "add": {"type": "boolean", "description": "Add to the selection instead of replacing it"}, "show_font_tab": {"type": "boolean", "description": "Switch to the font window (default true)"}}), &["names"], false),
        tool("select_points", "Select points", "Select nodes in the glyph being edited, as [contour, point] index pairs, for node commands like contour.break or contour.toggle_smooth.", json!({"points": {"type": "array", "items": {"type": "array", "items": {"type": "integer"}}}, "add": {"type": "boolean"}}), &["points"], false),
        tool("open_glyph", "Open glyph", "Open a glyph in a glyph-editing tab.", json!({"name": {"type": "string"}}), &["name"], false),
        tool("open_text", "Open text", "Open a text tab showing a line of text, with the text, metrics or kerning tool.", json!({"text": {"type": "string"}, "tool": {"type": "string", "enum": ["text", "metrics", "kerning"]}}), &["text"], false),
        tool("select_tab", "Select tab", "Make a tab active by index (see get_state).", json!({"index": {"type": "integer"}}), &["index"], false),
        tool("set_tool", "Set tool", "Pick a drawing tool: contour, element, metrics, kerning, text, pen, pencil, rectangle, ellipse, knife, eraser, anchor, measure, hand, zoom.", json!({"tool": {"type": "string"}}), &["tool"], false),
        tool("set_metrics", "Set metrics", "Set the left/right sidebearings and/or advance width of glyphs (default: the selected or current glyphs).", json!({"glyphs": {"type": "array", "items": {"type": "string"}}, "lsb": {"type": "number"}, "rsb": {"type": "number"}, "width": {"type": "number"}}), &[], false),
        tool("get_kerning", "Get kerning", "Kerning of one pair (including class kerning), or all pairs of the current master when left/right are omitted.", json!({"left": {"type": "string"}, "right": {"type": "string"}}), &[], true),
        tool("set_kerning", "Set kerning", "Set a kerning pair for the current master; 0 removes it. Use @ClassName for class kerning.", json!({"left": {"type": "string"}, "right": {"type": "string"}, "value": {"type": "number"}}), &["left", "right", "value"], false),
        tool("get_font_info", "Get font info", "Names, dimensions, legal fields, axes, instances and per-master metrics of the current font.", json!({}), &[], true),
        tool("set_font_info", "Set font info", "Change font info fields (e.g. {\"info\": {\"family\": \"Grotto\"}}) and/or master fields (e.g. {\"master\": {\"x_height\": 520}}). Field names are as returned by get_font_info.", json!({"info": {"type": "object"}, "master": {"type": "object"}, "master_index": {"type": "integer"}}), &[], false),
        tool("select_master", "Select master", "Make a master current by index.", json!({"index": {"type": "integer"}}), &["index"], false),
        tool("get_features", "Get features", "The OpenType feature code of the current font.", json!({}), &[], true),
        tool("set_features", "Set features", "Replace the OpenType feature code.", json!({"code": {"type": "string"}}), &["code"], false),
        tool("get_preferences", "Get preferences", "All app preferences (theme, view options, panels, cell size, nudge steps…).", json!({}), &[], true),
        tool("set_preferences", "Set preferences", "Change preferences, e.g. {\"fields\": {\"show_grid\": true, \"theme\": \"Light\"}}.", json!({"fields": {"type": "object"}}), &["fields"], false),
        tool("close_dialogs", "Close dialogs", "Close any open dialog or message window.", json!({}), &[], false),
        tool("screenshot", "Screenshot", "A PNG picture of the app window as it is now.", json!({"max_width": {"type": "integer", "description": "Scale down to at most this width (default 1280)"}}), &[], true),
    ]
}

// ------------------------------------------------------------------------------------------
// Tool calls (UI thread)

fn tool_text(text: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": text.into()}], "isError": false})
}

fn tool_json(v: Value) -> Value {
    json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}], "structuredContent": v, "isError": false})
}

fn tool_error(text: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": text.into()}], "isError": true})
}

/// Run every waiting agent call. Called once per frame before the UI is drawn.
pub fn pump(app: &mut App, ctx: &egui::Context) {
    let calls: Vec<Call> = app.mcp.rx.try_iter().collect();
    for call in calls {
        app.mcp.calls += 1;
        let now = ctx.input(|i| i.time);
        app.mcp.last_call = Some((call.name.clone(), now));
        if call.name == "screenshot" {
            let w = call.args.get("max_width").and_then(|v| v.as_u64()).unwrap_or(1280).clamp(200, 8000) as u32;
            app.mcp.pending_shots.push((call.reply, w));
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            continue;
        }
        let summary = summarize(&call.name, &call.args);
        let result = match call_tool(app, ctx, &call.name, &call.args) {
            Ok(v) => v,
            Err(e) => tool_error(e),
        };
        let failed = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        if !matches!(call.name.as_str(), "get_state" | "list_glyphs" | "get_glyph" | "list_commands" | "get_kerning" | "get_font_info" | "get_features" | "get_preferences") {
            app.log(format!("Agent: {summary}{}", if failed { " (failed)" } else { "" }));
        }
        let _ = call.reply.send(result);
        ctx.request_repaint();
    }
    if !app.mcp.pending_shots.is_empty() {
        ctx.request_repaint();
    }
}

/// Hand finished window screenshots to the agents that asked for them.
pub fn deliver_screenshots(app: &mut App, ctx: &egui::Context) {
    if app.mcp.pending_shots.is_empty() {
        return;
    }
    let shot = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    let Some(img) = shot else { return };
    let [w, h] = img.size;
    let mut buf = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        buf.extend_from_slice(&c.to_srgba_unmultiplied());
    }
    let Some(rgba) = image::RgbaImage::from_raw(w as u32, h as u32, buf) else { return };
    for (reply, max_w) in app.mcp.pending_shots.drain(..) {
        let pic = if rgba.width() > max_w {
            let nh = (rgba.height() as f32 * max_w as f32 / rgba.width() as f32).round().max(1.0) as u32;
            image::imageops::thumbnail(&rgba, max_w, nh)
        } else {
            rgba.clone()
        };
        let mut png = Vec::new();
        let res = match pic.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png) {
            Ok(()) => json!({"content": [{"type": "image", "data": base64(&png), "mimeType": "image/png"}], "isError": false}),
            Err(e) => tool_error(format!("Could not encode the screenshot: {e}")),
        };
        let _ = reply.send(res);
    }
}

fn summarize(name: &str, args: &Value) -> String {
    match name {
        "run_command" => format!("{}", args.get("id").and_then(|v| v.as_str()).unwrap_or("?")),
        _ => {
            let a = args.get("name").or_else(|| args.get("text")).or_else(|| args.get("tool")).and_then(|v| v.as_str());
            match a {
                Some(a) => format!("{name} {a}"),
                None => name.to_string(),
            }
        }
    }
}

fn s<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn f(args: &Value, key: &str) -> Option<f32> {
    args.get(key).and_then(|v| v.as_f64()).map(|v| v as f32)
}

fn names(args: &Value, key: &str) -> Vec<String> {
    args.get(key).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default()
}

fn glyph_idx(app: &App, name: &str) -> Result<usize, String> {
    let font = &app.doc().font;
    font.find(name)
        .or_else(|| {
            // Accept a single character or U+XXXX too.
            let mut cs = name.chars();
            let u = match (cs.next(), cs.next()) {
                (Some(c), None) => Some(c as u32),
                _ => name.strip_prefix("U+").and_then(|h| u32::from_str_radix(h, 16).ok()),
            };
            u.and_then(|u| font.find_unicode(u))
        })
        .ok_or_else(|| format!("No glyph named '{name}' in {}", font.title()))
}

/// Merge a JSON object of field changes into a serializable value.
fn merge<T: Serialize + DeserializeOwned>(v: &mut T, patch: &Value) -> Result<Vec<String>, String> {
    let Value::Object(p) = patch else { return Err("Expected an object of fields".into()) };
    let mut cur = serde_json::to_value(&*v).map_err(|e| e.to_string())?;
    let Value::Object(obj) = &mut cur else { return Err("Not an object".into()) };
    let mut changed = Vec::new();
    for (k, val) in p {
        if !obj.contains_key(k) {
            return Err(format!("Unknown field '{k}'"));
        }
        obj.insert(k.clone(), val.clone());
        changed.push(k.clone());
    }
    *v = serde_json::from_value(cur).map_err(|e| format!("Invalid value: {e}"))?;
    Ok(changed)
}

fn mark_changed(app: &mut App) {
    let d = app.doc_mut();
    d.font.dirty = true;
    d.view_key = u64::MAX;
}

fn call_tool(app: &mut App, ctx: &egui::Context, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "list_commands" => {
            let q = s(args, "filter").unwrap_or("").to_lowercase();
            let list: Vec<Value> = commands::COMMANDS
                .iter()
                .filter(|c| q.is_empty() || c.id.contains(&q) || c.label.to_lowercase().contains(&q))
                .map(|c| {
                    let mut m = Map::new();
                    m.insert("id".into(), json!(c.id));
                    m.insert("label".into(), json!(commands::label(app, c).trim_end_matches('…')));
                    if !c.keys.is_empty() {
                        m.insert("shortcut".into(), json!(c.keys));
                    }
                    m.insert("enabled".into(), json!(commands::enabled(app, c.when)));
                    if c.path != commands::PathArg::None {
                        m.insert("path".into(), json!(c.path.describe()));
                    }
                    Value::Object(m)
                })
                .collect();
            Ok(tool_json(json!({"commands": list})))
        }
        "run_command" => {
            let id = s(args, "id").ok_or("Missing id")?;
            let path = s(args, "path").map(PathBuf::from);
            let msg = commands::run(app, ctx, id, path, false)?;
            Ok(tool_text(if msg.is_empty() { format!("Ran {id}") } else { msg }))
        }
        "get_state" => Ok(tool_json(state(app))),
        "list_glyphs" => {
            let q = s(args, "filter").unwrap_or("").to_lowercase();
            let off = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let lim = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(300) as usize;
            let d = app.doc();
            let m = d.master;
            let all: Vec<&Glyph> = d.font.glyphs.iter().filter(|g| q.is_empty() || g.name.to_lowercase().contains(&q) || g.char().map(|c| q.contains(c)).unwrap_or(false)).collect();
            let rows: Vec<Value> = all
                .iter()
                .skip(off)
                .take(lim)
                .map(|g| {
                    let l = g.layer(m);
                    json!({"name": g.name, "unicode": g.unicode_str(), "width": l.width, "contours": l.contours.len(), "components": l.components.len(), "flag": g.flag.map(|i| crate::font::FLAG_COLORS[i.min(7)].3)})
                })
                .collect();
            Ok(tool_json(json!({"total": all.len(), "offset": off, "glyphs": rows})))
        }
        "get_glyph" => {
            let gi = glyph_idx(app, s(args, "name").ok_or("Missing name")?)?;
            let d = app.doc();
            let m = args.get("master").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(d.master).min(d.font.masters.len() - 1);
            let g = &d.font.glyphs[gi];
            let l = g.layer(m);
            Ok(tool_json(json!({
                "name": g.name,
                "unicodes": g.unicodes.iter().map(|u| format!("{u:04X}")).collect::<Vec<_>>(),
                "master": d.font.masters[m].name,
                "width": l.width,
                "lsb": d.font.lsb(gi, m),
                "rsb": d.font.rsb(gi, m),
                "contours": l.contours,
                "components": l.components,
                "anchors": l.anchors,
                "guides": l.guides,
                "note": g.note,
            })))
        }
        "set_glyph" => set_glyph(app, args),
        "add_glyphs" => {
            let n = names(args, "names");
            if n.is_empty() {
                return Err("Pass at least one name".into());
            }
            let before = app.doc().font.glyphs.len();
            app.add_glyphs_by_names(ctx, &n);
            Ok(tool_text(format!("Added {} glyph(s)", app.doc().font.glyphs.len() - before)))
        }
        "delete_glyphs" => {
            let idx: Vec<usize> = names(args, "names").iter().map(|n| glyph_idx(app, n)).collect::<Result<_, _>>()?;
            let di = app.doc_idx();
            app.docs[di].checkpoint_font("Delete glyphs");
            app.docs[di].font.remove_glyphs(&idx);
            app.docs[di].sel.clear();
            app.docs[di].focus = None;
            app.cache.clear_doc(di);
            mark_changed(app);
            Ok(tool_text(format!("Deleted {} glyph(s)", idx.len())))
        }
        "select_glyphs" => {
            let idx: Vec<usize> = names(args, "names").iter().map(|n| glyph_idx(app, n)).collect::<Result<_, _>>()?;
            if args.get("show_font_tab").and_then(|v| v.as_bool()).unwrap_or(true) && app.in_glyph_tab() {
                let di = app.doc_idx();
                if let Some(ti) = app.tabs.iter().position(|t| t.doc == di && t.kind == TabKind::Font) {
                    app.active = ti;
                }
            }
            let add = args.get("add").and_then(|v| v.as_bool()).unwrap_or(false);
            let d = app.doc_mut();
            if !add {
                d.sel.clear();
            }
            d.sel.extend(idx.iter().copied());
            d.focus = idx.last().copied().or(d.focus);
            d.anchor = d.focus;
            Ok(tool_text(format!("{} glyph(s) selected", d.sel.len())))
        }
        "select_points" => {
            let gi = if app.in_glyph_tab() { app.current_glyph() } else { None }.ok_or("Open a glyph first (open_glyph)")?;
            let add = args.get("add").and_then(|v| v.as_bool()).unwrap_or(false);
            let pts: Vec<(usize, usize)> = args.get("points").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|p| Some((p.get(0)?.as_u64()? as usize, p.get(1)?.as_u64()? as usize))).collect()).unwrap_or_default();
            let di = app.doc_idx();
            let m = app.docs[di].master;
            let l = app.docs[di].font.glyphs[gi].layer_mut(m);
            if !add {
                for c in &mut l.contours {
                    for p in &mut c.points {
                        p.sel = false;
                    }
                }
                for c in &mut l.components {
                    c.sel = false;
                }
                for a in &mut l.anchors {
                    a.sel = false;
                }
            }
            let mut n = 0;
            for (ci, pi) in pts {
                if let Some(p) = l.contours.get_mut(ci).and_then(|c| c.points.get_mut(pi)) {
                    p.sel = true;
                    n += 1;
                } else {
                    return Err(format!("No point [{ci}, {pi}]"));
                }
            }
            app.docs[di].font.glyphs[gi].touch();
            Ok(tool_text(format!("Selected {n} point(s)")))
        }
        "open_glyph" => {
            let gi = glyph_idx(app, s(args, "name").ok_or("Missing name")?)?;
            app.open_glyph_tab(gi);
            Ok(tool_text(format!("Editing {}", app.doc().font.glyphs[gi].name)))
        }
        "open_text" => {
            let text = s(args, "text").ok_or("Missing text")?;
            let tool = match s(args, "tool").unwrap_or("text") {
                "metrics" => Tool::Metrics,
                "kerning" => Tool::Kerning,
                _ => Tool::Text,
            };
            app.open_text_tab(text, tool);
            Ok(tool_text(format!("Showing \"{text}\"")))
        }
        "select_tab" => {
            let i = args.get("index").and_then(|v| v.as_u64()).ok_or("Missing index")? as usize;
            if i >= app.tabs.len() {
                return Err(format!("There are {} tabs", app.tabs.len()));
            }
            app.active = i;
            Ok(tool_text(format!("Tab {i} is active")))
        }
        "set_tool" => {
            let want = s(args, "tool").ok_or("Missing tool")?.to_lowercase();
            let t = Tool::ALL.iter().copied().find(|t| t.label().to_lowercase() == want || (want == "rapid" && *t == Tool::Rapid)).ok_or_else(|| format!("Unknown tool '{want}'"))?;
            app.tool = t;
            Ok(tool_text(format!("{} tool", t.label())))
        }
        "set_metrics" => {
            let list = names(args, "glyphs");
            let targets: Vec<usize> = if list.is_empty() {
                if app.in_glyph_tab() { app.current_glyph().into_iter().collect() } else { app.doc().selected() }
            } else {
                list.iter().map(|n| glyph_idx(app, n)).collect::<Result<_, _>>()?
            };
            if targets.is_empty() {
                return Err("No glyphs: pass \"glyphs\" or select some first".into());
            }
            let (lsb, rsb, width) = (f(args, "lsb"), f(args, "rsb"), f(args, "width"));
            if lsb.is_none() && rsb.is_none() && width.is_none() {
                return Err("Pass lsb, rsb and/or width".into());
            }
            let di = app.doc_idx();
            let m = app.docs[di].master;
            app.docs[di].checkpoint_font("Set metrics");
            for &gi in &targets {
                apply_metrics(&mut app.docs[di].font, gi, m, lsb, rsb, width);
                app.docs[di].font.glyphs[gi].touch();
            }
            mark_changed(app);
            Ok(tool_text(format!("Set metrics of {} glyph(s)", targets.len())))
        }
        "get_kerning" => {
            let d = app.doc();
            match (s(args, "left"), s(args, "right")) {
                (Some(l), Some(r)) => {
                    let v = d.font.kerning(d.master, l, r);
                    Ok(tool_json(json!({"left": l, "right": r, "value": v.as_ref().map(|k| k.0).unwrap_or(0.0), "from": v.map(|k| format!("{} {}", k.1, k.2))})))
                }
                _ => Ok(tool_json(json!({"master": d.font.masters[d.master].name, "pairs": d.font.masters[d.master].kerning}))),
            }
        }
        "set_kerning" => {
            let (l, r) = (s(args, "left").ok_or("Missing left")?, s(args, "right").ok_or("Missing right")?);
            let v = f(args, "value").ok_or("Missing value")?;
            for n in [l, r] {
                if !n.starts_with('@') {
                    glyph_idx(app, n)?;
                } else if !app.doc().font.classes.iter().any(|c| c.name == n[1..]) {
                    return Err(format!("No class named '{}'", &n[1..]));
                }
            }
            let di = app.doc_idx();
            let m = app.docs[di].master;
            app.docs[di].checkpoint_font("Set kerning");
            app.docs[di].font.set_kerning(m, l, r, v.round());
            mark_changed(app);
            Ok(tool_text(format!("Kerning {l} {r} = {}", v.round())))
        }
        "get_font_info" => {
            let d = app.doc();
            let masters: Vec<Value> = d
                .font
                .masters
                .iter()
                .map(|ms| {
                    let mut v = serde_json::to_value(ms).unwrap_or(Value::Null);
                    if let Value::Object(o) = &mut v {
                        let n = ms.kerning.len();
                        o.remove("kerning");
                        o.insert("kerning_pairs".into(), json!(n));
                    }
                    v
                })
                .collect();
            Ok(tool_json(json!({
                "info": d.font.info,
                "current_master": d.master,
                "masters": masters,
                "axes": d.font.axes,
                "instances": d.font.instances,
                "classes": d.font.classes,
                "glyph_count": d.font.glyphs.len(),
                "path": d.font.path,
            })))
        }
        "set_font_info" => {
            let di = app.doc_idx();
            app.docs[di].checkpoint_font("Font info");
            let mut changed = Vec::new();
            if let Some(info) = args.get("info") {
                changed.extend(merge(&mut app.docs[di].font.info, info)?);
            }
            if let Some(mp) = args.get("master") {
                let mi = args.get("master_index").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(app.docs[di].master);
                let ms = app.docs[di].font.masters.get_mut(mi).ok_or("No such master")?;
                changed.extend(merge(ms, mp)?.into_iter().map(|k| format!("master.{k}")));
            }
            if changed.is_empty() {
                return Err("Pass \"info\" and/or \"master\" fields".into());
            }
            for g in &mut app.docs[di].font.glyphs {
                g.touch();
            }
            mark_changed(app);
            Ok(tool_text(format!("Changed {}", changed.join(", "))))
        }
        "select_master" => {
            let i = args.get("index").and_then(|v| v.as_u64()).ok_or("Missing index")? as usize;
            let d = app.doc_mut();
            if i >= d.font.masters.len() {
                return Err(format!("The font has {} master(s)", d.font.masters.len()));
            }
            d.master = i;
            d.view_key = u64::MAX;
            Ok(tool_text(format!("Master {}", d.font.masters[i].name)))
        }
        "get_features" => Ok(tool_text(app.doc().font.features.clone())),
        "set_features" => {
            let code = s(args, "code").ok_or("Missing code")?.to_string();
            let di = app.doc_idx();
            app.docs[di].checkpoint_font("Features");
            app.docs[di].font.features = code;
            mark_changed(app);
            Ok(tool_text("Feature code replaced"))
        }
        "get_preferences" => Ok(tool_json(serde_json::to_value(&app.prefs).map_err(|e| e.to_string())?)),
        "set_preferences" => {
            let fields = args.get("fields").ok_or("Missing fields")?;
            let old_theme = app.prefs.theme;
            let mut p = app.prefs.clone();
            let changed = merge(&mut p, fields)?;
            app.prefs = p;
            if app.prefs.theme != old_theme {
                let t = app.prefs.theme;
                app.set_theme(ctx, t);
            }
            app.prefs_dirty = true;
            Ok(tool_text(format!("Changed {}", changed.join(", "))))
        }
        "close_dialogs" => {
            let d = &mut app.dlg;
            let open = open_dialogs(d);
            d.welcome = false;
            d.font_info = false;
            d.prefs = false;
            d.export = false;
            d.find = false;
            d.add_glyphs = false;
            d.rename = None;
            d.open_installed = false;
            d.kern_adjust = false;
            d.kern_remove = false;
            d.add_component = false;
            d.add_anchor = false;
            d.add_variation = false;
            d.instance = false;
            d.metrics_link = false;
            d.actions = false;
            d.shortcuts = false;
            d.about = false;
            d.message = None;
            d.coming_soon = None;
            d.confirm_close = None;
            d.confirm_quit = false;
            Ok(tool_text(if open.is_empty() { "No dialogs were open".to_string() } else { format!("Closed {}", open.join(", ")) }))
        }
        _ => Err(format!("Unknown tool '{name}'")),
    }
}

fn open_dialogs(d: &Dialogs) -> Vec<String> {
    let mut v: Vec<String> = [
        (d.welcome, "welcome"),
        (d.font_info, "font_info"),
        (d.prefs, "preferences"),
        (d.export, "export"),
        (d.find, "find"),
        (d.add_glyphs, "add_glyphs"),
        (d.rename.is_some(), "rename"),
        (d.open_installed, "open_installed"),
        (d.kern_adjust, "kern_adjust"),
        (d.kern_remove, "kern_remove"),
        (d.add_component, "add_component"),
        (d.add_anchor, "add_anchor"),
        (d.add_variation, "add_variation"),
        (d.instance, "generate_instance"),
        (d.metrics_link, "link_metrics"),
        (d.actions, "actions"),
        (d.shortcuts, "shortcuts"),
        (d.about, "about"),
        (d.confirm_close.is_some(), "confirm_close"),
        (d.confirm_quit, "confirm_quit"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, n)| n.to_string())
    .collect();
    if let Some((t, b)) = &d.message {
        v.push(format!("message: {t} — {b}"));
    }
    if let Some(m) = &d.coming_soon {
        v.push(format!("coming_soon: {m}"));
    }
    v
}

fn state(app: &App) -> Value {
    let d = app.doc();
    let tabs: Vec<Value> = app
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let font = app.docs.get(t.doc).map(|d| d.font.title()).unwrap_or_default();
            match t.kind {
                TabKind::Font => json!({"index": i, "kind": "font", "font": font, "active": i == app.active}),
                TabKind::Glyph => json!({"index": i, "kind": "glyph", "font": font, "text": t.text_input, "glyphs": t.text, "current": t.cur, "active": i == app.active}),
            }
        })
        .collect();
    let fonts: Vec<Value> = app.docs.iter().map(|d| json!({"title": d.font.title(), "glyphs": d.font.glyphs.len(), "masters": d.font.masters.iter().map(|m| m.name.clone()).collect::<Vec<_>>(), "unsaved": d.font.dirty, "path": d.font.path})).collect();
    let sel: Vec<String> = d.selected().iter().take(500).map(|&i| d.font.glyphs[i].name.clone()).collect();
    let mut selected_points = Vec::new();
    if app.in_glyph_tab() {
        if let Some(gi) = app.current_glyph() {
            for (ci, c) in d.font.glyphs[gi].layer(d.master).contours.iter().enumerate() {
                for (pi, p) in c.points.iter().enumerate() {
                    if p.sel {
                        selected_points.push(json!([ci, pi]));
                    }
                }
            }
        }
    }
    json!({
        "fonts": fonts,
        "current_font": app.doc_idx(),
        "tabs": tabs,
        "active_tab": app.active,
        "tool": app.tool.label(),
        "current_glyph": app.current_glyph().map(|gi| d.font.glyphs[gi].name.clone()),
        "current_master": d.font.masters[d.master].name,
        "selected_glyphs": sel,
        "selected_points": selected_points,
        "can_undo": d.undo.last().map(|u| u.label.clone()),
        "can_redo": d.redo.last().map(|u| u.label.clone()),
        "open_dialogs": open_dialogs(&app.dlg),
        "status": app.status.as_ref().map(|s| s.0.clone()),
        "recent_log": app.output.iter().rev().take(12).rev().cloned().collect::<Vec<_>>(),
    })
}

fn set_glyph(app: &mut App, args: &Value) -> Result<Value, String> {
    let name = s(args, "name").ok_or("Missing name")?.to_string();
    let di = app.doc_idx();
    let m = args.get("master").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(app.docs[di].master);
    if m >= app.docs[di].font.masters.len() {
        return Err("No such master".into());
    }
    // Parse everything before touching the font so a bad argument changes nothing.
    let contours: Option<Vec<Contour>> = match args.get("contours") {
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| format!("Bad contours: {e}"))?),
        None => None,
    };
    let components: Option<Vec<Component>> = match args.get("components") {
        Some(Value::Array(a)) => Some(
            a.iter()
                .map(|c| {
                    let base = c.get("base").and_then(|v| v.as_str()).ok_or("Component needs base")?.to_string();
                    let t: Vec<f32> = c.get("t").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_f64().map(|x| x as f32)).collect()).unwrap_or_else(|| vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                    if t.len() != 6 {
                        return Err("Component t needs 6 numbers".to_string());
                    }
                    Ok(Component { base, t: [t[0], t[1], t[2], t[3], t[4], t[5]], sel: false })
                })
                .collect::<Result<_, String>>()?,
        ),
        Some(_) => return Err("components must be a list".into()),
        None => None,
    };
    if let Some(cs) = &components {
        for c in cs {
            if app.docs[di].font.find(&c.base).is_none() {
                return Err(format!("Component base '{}' is not in the font", c.base));
            }
        }
    }
    let anchors: Option<Vec<Anchor>> = match args.get("anchors") {
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| format!("Bad anchors: {e}"))?),
        None => None,
    };
    let unicodes: Option<Vec<u32>> = match args.get("unicodes") {
        Some(Value::Array(a)) => Some(a.iter().map(|u| u.as_str().and_then(|h| u32::from_str_radix(h.trim_start_matches("U+"), 16).ok()).ok_or_else(|| format!("Bad Unicode value {u}"))).collect::<Result<_, _>>()?),
        Some(_) => return Err("unicodes must be a list of hex strings".into()),
        None => None,
    };

    let existing = app.docs[di].font.find(&name);
    let gi = match existing {
        Some(gi) => {
            app.docs[di].checkpoint_glyph(gi, "Agent edit");
            gi
        }
        None => {
            app.docs[di].checkpoint_font("Agent new glyph");
            let masters = app.docs[di].font.masters.len();
            let w = (app.docs[di].font.info.upm * 0.6).round();
            let u = crate::glyphnames::unicode_for_name(&name);
            app.docs[di].font.add_glyph(Glyph::new(&name, u.into_iter().collect(), masters, w))
        }
    };
    let mut renamed = None;
    {
        let font = &mut app.docs[di].font;
        let g = &mut font.glyphs[gi];
        if let Some(u) = unicodes {
            g.unicodes = u;
        }
        if let Some(n) = s(args, "note") {
            g.note = n.to_string();
        }
        let l = g.layer_mut(m);
        if let Some(c) = contours {
            l.contours = c;
        }
        if let Some(c) = components {
            l.components = c;
        }
        if let Some(a) = anchors {
            l.anchors = a;
        }
        if let Some(w) = f(args, "width") {
            l.width = w;
        }
        g.touch();
        if let Some(nn) = s(args, "new_name") {
            if nn != name {
                if !font.rename_glyph(gi, nn) {
                    return Err(format!("Could not rename to '{nn}' (the name is taken or empty); other changes were kept"));
                }
                renamed = Some(nn.to_string());
            }
        }
    }
    if let Some(nn) = &renamed {
        for t in &mut app.tabs {
            for n in &mut t.text {
                if *n == name {
                    *n = nn.clone();
                }
            }
        }
    }
    mark_changed(app);
    let g = &app.docs[di].font.glyphs[gi];
    let l = g.layer(m);
    Ok(tool_text(format!(
        "{} {}: width {}, {} contour(s), {} component(s), {} anchor(s)",
        if existing.is_some() { "Updated" } else { "Created" },
        g.name,
        l.width,
        l.contours.len(),
        l.components.len(),
        l.anchors.len()
    )))
}

/// Standard base64 (for PNG screenshots).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_reference() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn protocol_basics_answer_without_the_ui() {
        let (tx, _rx) = channel();
        let ctx = egui::Context::default();
        let init = handle_text(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#, &tx, &ctx).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(handle_text(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, &tx, &ctx).is_none());
        let list = handle_text(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, &tx, &ctx).unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "run_command"));
        let bad = handle_text(r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#, &tx, &ctx).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
        // The local HTTP endpoint answers JSON-RPC and refuses web-page origins.
        let mut b = Bridge::new(&ctx);
        let port = 47_571;
        b.start_http(port);
        assert!(b.http_error.is_none(), "{:?}", b.http_error);
        let post = |origin: &str| {
            let body = r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#;
            let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let req = ["POST /mcp HTTP/1.1", "Host: 127.0.0.1", &format!("{origin}Content-Type: application/json"), &format!("Content-Length: {}", body.len()), "Connection: close", "", body].join("\r\n");
            c.write_all(req.as_bytes()).unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).unwrap();
            out
        };
        let ok = post("");
        assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
        assert!(ok.contains(r#""id":7"#));
        assert!(post("Origin: https://evil.example\r\n").starts_with("HTTP/1.1 403"));
        b.stop_http();
        // Every command id in the menus is unique.
        let mut ids: Vec<&str> = commands::COMMANDS.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len());
    }
}
