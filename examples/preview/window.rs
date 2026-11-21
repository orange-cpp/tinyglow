use crate::scene::{HEIGHT, Scene, WIDTH};
use std::cell::Cell;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tinyglow::{ClassicGlow, Quality};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_BTNFACE, DEFAULT_GUI_FONT, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Error, HRESULT, PCWSTR, Result, w};

const MARGIN: i32 = 16;
const VIEW_TOP: i32 = 42;
const TOOLBAR_TOP: i32 = VIEW_TOP + HEIGHT as i32 + MARGIN;
const CLIENT_HEIGHT: i32 = TOOLBAR_TOP + 78;
const SS_BLACKRECT: WINDOW_STYLE = WINDOW_STYLE(0x0004);
const COLORS: [(&str, [u8; 3]); 5] = [
    ("Cyan", [72, 220, 255]),
    ("Amber", [255, 173, 67]),
    ("Magenta", [255, 91, 188]),
    ("Lime", [154, 255, 88]),
    ("White", [255, 255, 255]),
];

#[derive(Clone, Copy)]
#[repr(usize)]
enum Command {
    Toggle = 1,
    Quality,
    Color,
    Thinner,
    Thicker,
    Dimmer,
    Brighter,
    Pause,
    Reset,
}

const BUTTONS: [(Command, &str, i32); 9] = [
    (Command::Toggle, "Glow on", 90),
    (Command::Quality, "Full resolution", 114),
    (Command::Color, "Color: Cyan", 117),
    (Command::Thinner, "Thickness −", 104),
    (Command::Thicker, "Thickness +", 104),
    (Command::Dimmer, "Intensity −", 94),
    (Command::Brighter, "Intensity +", 94),
    (Command::Pause, "Pause", 76),
    (Command::Reset, "Reset", 72),
];

thread_local! {
    static COMMAND: Cell<Option<Command>> = const { Cell::new(None) };
}

struct Settings {
    glow: ClassicGlow,
    enabled: bool,
    color: usize,
    paused: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            glow: ClassicGlow {
                color: COLORS[0].1,
                ..ClassicGlow::default()
            },
            enabled: true,
            color: 0,
            paused: false,
        }
    }
}

impl Settings {
    fn apply(&mut self, command: Command) {
        match command {
            Command::Toggle => self.enabled = !self.enabled,
            Command::Quality => {
                self.glow.quality = match self.glow.quality {
                    Quality::Full => Quality::Half,
                    Quality::Half => Quality::Full,
                }
            }
            Command::Color => {
                self.color = (self.color + 1) % COLORS.len();
                self.glow.color = COLORS[self.color].1;
            }
            Command::Thinner => self.glow.thickness = (self.glow.thickness - 1.0).max(0.0),
            Command::Thicker => self.glow.thickness = (self.glow.thickness + 1.0).min(100.0),
            Command::Dimmer => self.glow.intensity = (self.glow.intensity - 0.1).max(0.1),
            Command::Brighter => self.glow.intensity = (self.glow.intensity + 0.1).min(3.0),
            Command::Pause => self.paused = !self.paused,
            Command::Reset => *self = Self::default(),
        }
    }
}

struct Window {
    main: HWND,
    viewport: HWND,
    buttons: Vec<HWND>,
    status: HWND,
}

impl Window {
    fn new() -> Result<Self> {
        let name = w!("TinyglowPreview");
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
        // All handles are created and used on this UI thread.
        unsafe {
            let _ = SetProcessDPIAware();
            let module = GetModuleHandleW(PCWSTR::null())?;
            let instance = HINSTANCE(module.0);
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: HBRUSH((COLOR_BTNFACE.0 as usize + 1) as *mut _),
                lpszClassName: name,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(Error::from_win32());
            }
            let mut bounds = RECT {
                right: WIDTH as i32 + 2 * MARGIN,
                bottom: CLIENT_HEIGHT,
                ..Default::default()
            };
            AdjustWindowRectEx(&mut bounds, style, false, WINDOW_EX_STYLE::default())?;
            let main = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                name,
                w!("Tinyglow Preview"),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                None,
                None,
                Some(instance),
                None,
            )?;
            let window = Controls {
                parent: main,
                instance,
            };
            window.create(
                w!("STATIC"),
                "Original",
                WS_VISIBLE,
                0,
                [MARGIN, 15, WIDTH as i32 / 2, 20],
            )?;
            window.create(
                w!("STATIC"),
                "Classic glow",
                WS_VISIBLE,
                0,
                [MARGIN + WIDTH as i32 / 2, 15, WIDTH as i32 / 2, 20],
            )?;
            let viewport = window.create(
                w!("STATIC"),
                "",
                WS_VISIBLE | SS_BLACKRECT,
                0,
                [MARGIN, VIEW_TOP, WIDTH as i32, HEIGHT as i32],
            )?;
            let mut x = MARGIN;
            let mut buttons = Vec::with_capacity(BUTTONS.len());
            for (command, label, width) in BUTTONS {
                buttons.push(window.create(
                    w!("BUTTON"),
                    label,
                    WS_VISIBLE | WS_TABSTOP,
                    command as usize,
                    [x, TOOLBAR_TOP, width, 28],
                )?);
                x += width + 8;
            }
            let status = window.create(
                w!("STATIC"),
                "",
                WS_VISIBLE,
                0,
                [MARGIN, TOOLBAR_TOP + 42, WIDTH as i32, 22],
            )?;
            Ok(Self {
                main,
                viewport,
                buttons,
                status,
            })
        }
    }

    fn update(&self, settings: &Settings) -> Result<()> {
        let quality = match settings.glow.quality {
            Quality::Full => "Full resolution",
            Quality::Half => "Half resolution",
        };
        set_text(
            self.buttons[Command::Toggle as usize - 1],
            if settings.enabled {
                "Glow on"
            } else {
                "Glow off"
            },
        )?;
        set_text(self.buttons[Command::Quality as usize - 1], quality)?;
        set_text(
            self.buttons[Command::Color as usize - 1],
            &format!("Color: {}", COLORS[settings.color].0),
        )?;
        set_text(
            self.buttons[Command::Pause as usize - 1],
            if settings.paused { "Resume" } else { "Pause" },
        )?;
        set_text(
            self.status,
            &format!(
                "Thickness {:.0}     Intensity {:.1}     Direct3D 9",
                settings.glow.thickness, settings.glow.intensity,
            ),
        )
    }
}

struct Controls {
    parent: HWND,
    instance: HINSTANCE,
}

impl Controls {
    fn create(
        &self,
        class: PCWSTR,
        label: &str,
        style: WINDOW_STYLE,
        id: usize,
        [x, y, width, height]: [i32; 4],
    ) -> Result<HWND> {
        let text = wide(label);
        // The control copies its text during creation. The font is a stock object.
        unsafe {
            let control = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                PCWSTR(text.as_ptr()),
                style | WS_CHILD,
                x,
                y,
                width,
                height,
                Some(self.parent),
                (id != 0).then_some(HMENU(id as *mut _)),
                Some(self.instance),
                None,
            )?;
            let font = GetStockObject(DEFAULT_GUI_FONT);
            SendMessageW(
                control,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
            Ok(control)
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_COMMAND if wparam.0 >> 16 == BN_CLICKED as usize => {
            let button = BUTTONS
                .iter()
                .find(|(command, _, _)| *command as usize == wparam.0);
            if let Some(&(command, _, _)) = button {
                COMMAND.set(Some(command));
                return LRESULT(0);
            }
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            return LRESULT(0);
        }
        _ => {}
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

pub fn run() -> Result<()> {
    let window = Window::new()?;
    let mut settings = Settings::default();
    window.update(&settings)?;
    let mut scene = Some(Scene::new(window.main, window.viewport)?);
    let mut angle = 0.0;
    let mut last_frame = Instant::now();
    unsafe {
        let _ = ShowWindow(window.main, SW_SHOW);
    }
    loop {
        let mut message = MSG::default();
        // Dispatch this thread's window events before encoding the next frame.
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            if message.message == WM_QUIT {
                return Ok(());
            }
            unsafe {
                if !IsDialogMessageW(window.main, &message).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if let Some(command) = COMMAND.take() {
                settings.apply(command);
                window.update(&settings)?;
            }
        }
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        if unsafe { IsIconic(window.main) }.as_bool() {
            sleep(Duration::from_millis(75));
            continue;
        }
        if !settings.paused {
            angle = (angle + elapsed.as_secs_f32().min(0.05) * 0.9) % std::f32::consts::TAU;
        }
        let draw = match &mut scene {
            Some(scene) => scene.render(angle, settings.enabled.then_some(settings.glow)),
            None => Scene::new(window.main, window.viewport).map(|renderer| scene = Some(renderer)),
        };
        if let Err(error) = draw {
            const DEVICE_LOST: HRESULT = HRESULT(0x8876_0868_u32 as i32);
            const DEVICE_NOT_RESET: HRESULT = HRESULT(0x8876_0869_u32 as i32);
            if !matches!(error.code(), DEVICE_LOST | DEVICE_NOT_RESET) {
                return Err(error);
            }
            scene = None;
            sleep(Duration::from_millis(150));
        }
    }
}

pub fn report_error(message: &str) {
    let text = wide(message);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            w!("Tinyglow Preview"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn set_text(window: HWND, text: &str) -> Result<()> {
    let text = wide(text);
    unsafe { SetWindowTextW(window, PCWSTR(text.as_ptr())) }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
