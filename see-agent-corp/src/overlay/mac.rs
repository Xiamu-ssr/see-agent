use std::sync::mpsc;
use std::time::Instant;

use objc2::class;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::sel;
use objc2::Encode;

use super::types::DrawCommand;
use super::Overlay;

/// Fade-out duration in seconds after command expires.
const FADE_DURATION: f64 = 0.3;

/// Interval in seconds for the timer that polls the command channel.
const TIMER_INTERVAL: f64 = 0.016; // ~60fps

enum OverlayMsg {
    Draw(DrawCommand),
    Dismiss,
    Quit,
}

struct ActiveCommand {
    command: DrawCommand,
    created: Instant,
    duration: f64,
}

impl ActiveCommand {
    fn is_expired(&self) -> bool {
        self.created.elapsed().as_secs_f64() > self.duration + FADE_DURATION
    }

    fn alpha(&self) -> f64 {
        let elapsed = self.created.elapsed().as_secs_f64();
        if elapsed < self.duration {
            1.0
        } else {
            let fade_elapsed = elapsed - self.duration;
            (1.0 - fade_elapsed / FADE_DURATION).max(0.0)
        }
    }
}

/// macOS overlay using AppKit transparent window.
///
/// Spawns a dedicated thread that creates an NSWindow with:
/// - Transparent background, click-through, not captured by screenshots
/// - Always on top (level 1000), visible on all Spaces
pub struct MacOverlay {
    sender: mpsc::Sender<OverlayMsg>,
}

impl Default for MacOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOverlay {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            overlay_thread(rx);
        });
        Self { sender: tx }
    }
}

impl Overlay for MacOverlay {
    fn show(&self, command: DrawCommand) {
        let _ = self.sender.send(OverlayMsg::Draw(command));
    }
    fn dismiss(&self) {
        let _ = self.sender.send(OverlayMsg::Dismiss);
    }
}

impl Drop for MacOverlay {
    fn drop(&mut self) {
        let _ = self.sender.send(OverlayMsg::Quit);
    }
}

// ---------------------------------------------------------------------------
// Overlay thread — all AppKit calls happen here
// ---------------------------------------------------------------------------

thread_local! {
    static OVERLAY_STATE_PTR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct OverlayState {
    rx: mpsc::Receiver<OverlayMsg>,
    commands: Vec<ActiveCommand>,
    view: *mut AnyObject,
    window: *mut AnyObject,
    should_quit: bool,
}

fn overlay_thread(rx: mpsc::Receiver<OverlayMsg>) {
    // SAFETY: All AppKit calls happen on this dedicated thread.
    // We use raw pointers because we're interacting with Objective-C runtime.

    // Ensure NSApplication exists
    let app: *mut AnyObject = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    let _: () = unsafe { msg_send![app, setActivationPolicy: 1_isize] }; // Accessory

    // Get main screen frame
    let screen: *mut AnyObject = unsafe { msg_send![class!(NSScreen), mainScreen] };
    let frame: CGRect = unsafe { msg_send![screen, frame] };

    // Create overlay window
    let window = create_overlay_window(frame);

    // Get content view and enable layers
    let content_view: *mut AnyObject = unsafe { msg_send![window, contentView] };
    let _: () = unsafe { msg_send![content_view, setWantsLayer: Bool::YES] };

    // Create custom view
    let view_class = register_overlay_view_class();
    let view: *mut AnyObject = unsafe { msg_send![view_class, alloc] };
    let view: *mut AnyObject = unsafe { msg_send![view, initWithFrame: frame] };
    let _: () = unsafe { msg_send![content_view, addSubview: view] };

    // Store state in thread-local for callback access
    let state = Box::leak(Box::new(OverlayState {
        rx,
        commands: Vec::new(),
        view,
        window,
        should_quit: false,
    }));
    let state_ptr = state as *mut OverlayState as usize;
    OVERLAY_STATE_PTR.with(|cell| cell.set(state_ptr));

    // Create timer that fires timerFired: on the view
    let timer: *mut AnyObject = unsafe {
        msg_send![
            class!(NSTimer),
            timerWithTimeInterval: TIMER_INTERVAL,
            target: view,
            selector: sel!(timerFired:),
            userInfo: std::ptr::null::<AnyObject>(),
            repeats: Bool::YES
        ]
    };

    let run_loop: *mut AnyObject = unsafe { msg_send![class!(NSRunLoop), currentRunLoop] };
    let mode = c"kCFRunLoopDefaultMode";
    let ns_mode: *mut AnyObject =
        unsafe { msg_send![class!(NSString), stringWithUTF8String: mode.as_ptr()] };
    let _: () = unsafe { msg_send![run_loop, addTimer: timer, forMode: ns_mode] };

    // Run event loop
    loop {
        let date: *mut AnyObject =
            unsafe { msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: 0.1_f64] };
        let _: () = unsafe { msg_send![run_loop, runUntilDate: date] };
        if state.should_quit {
            break;
        }
    }

    let _: () = unsafe { msg_send![window, close] };
}

fn create_overlay_window(frame: CGRect) -> *mut AnyObject {
    let alloc: *mut AnyObject = unsafe { msg_send![class!(NSWindow), alloc] };
    let window: *mut AnyObject = unsafe {
        msg_send![
            alloc,
            initWithContentRect: frame,
            styleMask: 0_usize,
            backing: 2_usize,
            defer: Bool::NO
        ]
    };

    let clear: *mut AnyObject = unsafe { msg_send![class!(NSColor), clearColor] };
    let _: () = unsafe { msg_send![window, setBackgroundColor: clear] };
    let _: () = unsafe { msg_send![window, setOpaque: Bool::NO] };
    let _: () = unsafe { msg_send![window, setIgnoresMouseEvents: Bool::YES] };
    let _: () = unsafe { msg_send![window, setLevel: 1000_isize] };
    let _: () = unsafe { msg_send![window, setCollectionBehavior: (1_usize | (1 << 4))] };
    let _: () = unsafe { msg_send![window, setSharingType: 0_usize] };
    let _: () = unsafe { msg_send![window, setHasShadow: Bool::NO] };
    let _: () =
        unsafe { msg_send![window, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()] };

    window
}

fn register_overlay_view_class() -> &'static AnyClass {
    static CLASS: std::sync::OnceLock<&'static AnyClass> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let superclass = class!(NSView);
        let name = c"SeeOverlayView";
        let mut builder = ClassBuilder::new(name, superclass).unwrap();

        unsafe {
            builder.add_method(
                sel!(isFlipped),
                is_flipped as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool,
            );
            builder.add_method(
                sel!(drawRect:),
                draw_rect as unsafe extern "C" fn(*mut AnyObject, Sel, CGRect),
            );
            builder.add_method(
                sel!(timerFired:),
                timer_fired as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
        }

        builder.register()
    })
}

unsafe extern "C" fn is_flipped(_this: *mut AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}

unsafe extern "C" fn timer_fired(_this: *mut AnyObject, _sel: Sel, _timer: *mut AnyObject) {
    OVERLAY_STATE_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr == 0 {
            return;
        }
        let state = unsafe { &mut *(ptr as *mut OverlayState) };

        while let Ok(msg) = state.rx.try_recv() {
            match msg {
                OverlayMsg::Draw(cmd) => {
                    let duration = cmd.duration();
                    state.commands.push(ActiveCommand {
                        command: cmd,
                        created: Instant::now(),
                        duration,
                    });
                }
                OverlayMsg::Dismiss => state.commands.clear(),
                OverlayMsg::Quit => {
                    state.should_quit = true;
                    return;
                }
            }
        }

        state.commands.retain(|c| !c.is_expired());

        let _: () = unsafe { msg_send![state.view, setNeedsDisplay: Bool::YES] };

        if state.commands.is_empty() {
            let _: () = unsafe { msg_send![state.window, setAlphaValue: 0.0_f64] };
        } else {
            let max_alpha = state.commands.iter().map(|c| c.alpha()).fold(0.0_f64, f64::max);
            let _: () = unsafe { msg_send![state.window, setAlphaValue: max_alpha] };
        }
    });
}

unsafe extern "C" fn draw_rect(_this: *mut AnyObject, _sel: Sel, _rect: CGRect) {
    OVERLAY_STATE_PTR.with(|cell| {
        let ptr = cell.get();
        if ptr == 0 {
            return;
        }
        let state = unsafe { &*(ptr as *mut OverlayState) };
        for active in &state.commands {
            let alpha = active.alpha() as f32;
            draw_command(&active.command, alpha);
        }
    });
}

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

fn draw_command(cmd: &DrawCommand, alpha: f32) {
    match cmd {
        DrawCommand::Click { x, y, double } => {
            let r = if *double { 20.0 } else { 12.0 };
            draw_circle(*x, *y, r, alpha);
            draw_label(*x + 20.0, *y - 10.0, "click", alpha);
        }
        DrawCommand::Type { text } => {
            draw_label(100.0, 50.0, &format!("⌨ {text}"), alpha);
        }
        DrawCommand::Drag { x1, y1, x2, y2 } => {
            draw_circle(*x1, *y1, 8.0, alpha);
            draw_line(*x1, *y1, *x2, *y2, alpha);
            draw_circle(*x2, *y2, 8.0, alpha);
            draw_label(*x2 + 15.0, *y2 - 10.0, "drag", alpha);
        }
        DrawCommand::Scroll { x, y, direction, amount } => {
            let arrow = match direction.as_str() {
                "up" => "↑",
                "down" => "↓",
                "left" => "←",
                "right" => "→",
                _ => "↕",
            };
            draw_label(*x, *y, &format!("{arrow} scroll {amount}"), alpha);
        }
        DrawCommand::Hotkey { keys } => {
            draw_label(100.0, 50.0, &format!("⌨ {}", keys.join(" + ")), alpha);
        }
        DrawCommand::Shell { command } => {
            let s = if command.len() > 60 {
                format!("$ {}…", &command[..57])
            } else {
                format!("$ {command}")
            };
            draw_label(100.0, 50.0, &s, alpha);
        }
        DrawCommand::Wait { seconds } => {
            draw_label(100.0, 50.0, &format!("⏳ wait {seconds:.1}s"), alpha);
        }
        DrawCommand::Screenshot => {
            draw_label(100.0, 50.0, "📷 screenshot", alpha);
        }
        DrawCommand::CallUser { question } => {
            let s = if question.len() > 80 {
                format!("❓ {}…", &question[..77])
            } else {
                format!("❓ {question}")
            };
            draw_label(100.0, 80.0, &s, alpha);
        }
        DrawCommand::Finished { summary } => {
            let s = if summary.len() > 80 {
                format!("✅ {}…", &summary[..77])
            } else {
                format!("✅ {summary}")
            };
            draw_label_colored(100.0, 80.0, &s, alpha, 0.2, 0.8, 0.2);
        }
    }
}

fn draw_circle(x: f64, y: f64, radius: f64, alpha: f32) {
    let rect = CGRect {
        origin: CGPoint { x: x - radius, y: y - radius },
        size: CGSize { width: radius * 2.0, height: radius * 2.0 },
    };
    let path: *mut AnyObject =
        unsafe { msg_send![class!(NSBezierPath), bezierPathWithOvalInRect: rect] };
    let color: *mut AnyObject = unsafe {
        msg_send![class!(NSColor), colorWithRed: 1.0_f64, green: 0.2_f64, blue: 0.2_f64, alpha: alpha as f64]
    };
    let _: () = unsafe { msg_send![color, setFill] };
    let _: () = unsafe { msg_send![path, fill] };
}

fn draw_line(x1: f64, y1: f64, x2: f64, y2: f64, alpha: f32) {
    let path: *mut AnyObject = unsafe { msg_send![class!(NSBezierPath), bezierPath] };
    let _: () = unsafe { msg_send![path, moveToPoint: CGPoint { x: x1, y: y1 }] };
    let _: () = unsafe { msg_send![path, lineToPoint: CGPoint { x: x2, y: y2 }] };
    let _: () = unsafe { msg_send![path, setLineWidth: 2.0_f64] };
    let color: *mut AnyObject = unsafe {
        msg_send![class!(NSColor), colorWithRed: 1.0_f64, green: 0.2_f64, blue: 0.2_f64, alpha: alpha as f64]
    };
    let _: () = unsafe { msg_send![color, setStroke] };
    let _: () = unsafe { msg_send![path, stroke] };
}

fn draw_label(x: f64, y: f64, text: &str, alpha: f32) {
    draw_label_colored(x, y, text, alpha, 0.9, 0.1, 0.1);
}

fn draw_label_colored(x: f64, y: f64, text: &str, alpha: f32, r: f64, g: f64, b: f64) {
    let c_str = std::ffi::CString::new(text).unwrap_or_default();
    let ns_string: *mut AnyObject =
        unsafe { msg_send![class!(NSString), stringWithUTF8String: c_str.as_ptr()] };

    let padding = 6.0;
    let font_size = 14.0;
    let text_width = text.len() as f64 * font_size * 0.55;
    let bg_rect = CGRect {
        origin: CGPoint { x: x - padding, y: y - padding },
        size: CGSize { width: text_width + padding * 2.0, height: font_size + padding * 2.0 },
    };

    let bg_path: *mut AnyObject = unsafe {
        msg_send![class!(NSBezierPath), bezierPathWithRoundedRect: bg_rect, xRadius: 4.0_f64, yRadius: 4.0_f64]
    };
    let bg_color: *mut AnyObject = unsafe {
        msg_send![class!(NSColor), colorWithRed: r, green: g, blue: b, alpha: (alpha * 0.85) as f64]
    };
    let _: () = unsafe { msg_send![bg_color, setFill] };
    let _: () = unsafe { msg_send![bg_path, fill] };

    let font: *mut AnyObject =
        unsafe { msg_send![class!(NSFont), boldSystemFontOfSize: font_size] };
    let white: *mut AnyObject = unsafe {
        msg_send![class!(NSColor), colorWithRed: 1.0_f64, green: 1.0_f64, blue: 1.0_f64, alpha: alpha as f64]
    };

    let font_key = c"NSFont";
    let color_key = c"NSColor";
    let k1: *mut AnyObject =
        unsafe { msg_send![class!(NSString), stringWithUTF8String: font_key.as_ptr()] };
    let k2: *mut AnyObject =
        unsafe { msg_send![class!(NSString), stringWithUTF8String: color_key.as_ptr()] };
    let keys: [*mut AnyObject; 2] = [k1, k2];
    let values: [*mut AnyObject; 2] = [font, white];
    let attrs: *mut AnyObject = unsafe {
        msg_send![class!(NSDictionary), dictionaryWithObjects: values.as_ptr(), forKeys: keys.as_ptr(), count: 2_usize]
    };

    let point = CGPoint { x, y };
    let _: () = unsafe { msg_send![ns_string, drawAtPoint: point, withAttributes: attrs] };
}

// ---------------------------------------------------------------------------
// CGRect / CGPoint / CGSize with Encode implementations
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

// SAFETY: These have the same layout as Cocoa's CGPoint/CGSize/CGRect.
unsafe impl Encode for CGPoint {
    const ENCODING: objc2::Encoding =
        objc2::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for CGSize {
    const ENCODING: objc2::Encoding =
        objc2::Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for CGRect {
    const ENCODING: objc2::Encoding =
        objc2::Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
}
unsafe impl objc2::RefEncode for CGPoint {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&Self::ENCODING);
}
unsafe impl objc2::RefEncode for CGSize {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&Self::ENCODING);
}
unsafe impl objc2::RefEncode for CGRect {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&Self::ENCODING);
}
