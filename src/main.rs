#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_json;

use std::collections::HashMap;
use std::rc::Weak;
use std::sync::{Arc, Mutex};

use druid::{AppLauncher, Data, Lens, UnitPoint, WidgetExt, WindowDesc, AppDelegate, Target, Command, DelegateCtx, Handled, Selector};
use druid::widget::{Flex, Label, TextBox};
use druid::widget::prelude::*;
use serde_json::Value;

use crate::rpc::{Core, Handler};
use crate::xi_thread::start_xi_thread;
use std::thread;

pub mod xi_thread;
pub mod rpc;


const VERTICAL_WIDGET_SPACING: f64 = 20.0;
const TEXT_BOX_WIDTH: f64 = 200.0;

pub type Id = usize;

#[derive(Clone, Data, Lens)]
struct HelloState {
    name: String,
}

fn build_root_widget() -> impl Widget<HelloState> {
    // a label that will determine its text based on the current app data.
    let label = Label::new(|data: &HelloState, _env: &Env| {
        if data.name.is_empty() {
            "Hello anybody!?".to_string()
        } else {
            format!("Hello {}!", data.name)
        }
    })
        .with_text_size(32.0);

    // a textbox that modifies `name`.
    let textbox = TextBox::new()
        .with_placeholder("Who are we greeting?")
        .with_text_size(18.0)
        .fix_width(TEXT_BOX_WIDTH)
        .lens(HelloState::name);

    // arrange the two widgets vertically, with some padding
    Flex::column()
        .with_child(label)
        .with_spacer(VERTICAL_WIDGET_SPACING)
        .with_child(textbox)
        .align_vertical(UnitPoint::CENTER)
}

type ViewId = String;

/// The commands the EditView widget accepts through `poke`.
pub enum EditViewCommands {
    ViewId(String),
    ApplyUpdate(Value),
    ScrollTo(usize),
    Core(Weak<Mutex<Core>>),
    Undo,
    Redo,
    UpperCase,
    LowerCase,
    Transpose,
    AddCursorAbove,
    AddCursorBelow,
    SingleSelection,
    SelectAll,
}


#[derive(Clone, Data)]
struct ViewState {
    id: Id,
    filename: Option<String>,
}

#[derive(Clone)]
struct AppState {
    focused: Option<ViewId>,
    views: HashMap<ViewId, ViewState>,
}

impl AppState {
    fn new() -> AppState {
        AppState {
            focused: Default::default(),
            views: HashMap::new(),
        }
    }

    fn get_focused(&self) -> String {
        self.focused.clone().expect("no focused viewstate")
    }

    fn get_focused_viewstate(&mut self) -> &mut ViewState {
        let view_id = self.focused.clone().expect("no focused viewstate");
        self.views.get_mut(&view_id).expect("Focused viewstate not found in views")
    }
}

#[derive(Clone)]
struct App {
    core: Arc<Mutex<Core>>,
    state: Arc<Mutex<AppState>>,
}

impl App {
    fn new(core: Core) -> App {
        App {
            core: Arc::new(Mutex::new(core)),
            state: Arc::new(Mutex::new(AppState::new())),
        }
    }

    fn send_notification(&self, method: &str, params: &Value) {
        self.get_core().send_notification(method, params);
    }

    fn send_view_cmd(&self, cmd: EditViewCommands) {
        let mut state = self.get_state();
        let focused = state.get_focused_viewstate();
    }
}

impl App {
    fn get_core(&self) -> std::sync::MutexGuard<'_, rpc::Core, > {
        self.core.lock().unwrap()
    }

    fn get_state(&self) -> std::sync::MutexGuard<'_, AppState, > {
        self.state.lock().unwrap()
    }
}

impl App {
    fn req_new_view(&self, filename: Option<&str>) {
        let mut params = json!({});

        let filename = if filename.is_some() {
            params["file_path"] = json!(filename.unwrap());
            Some(filename.unwrap().to_string())
        } else {
            None
        };

        let edit_view = 0;
        let core = Arc::downgrade(&self.core);
        let state = self.state.clone();

        self.core.lock().unwrap()
            .send_request("new_view", &params,
                          move |value| {
                              let view_id = value.clone().as_str().unwrap().to_string();
                              let mut state = state.lock().unwrap();
                              state.focused = Some(view_id.clone());
                          },
            );
    }

    fn handle_cmd(&self, method: &str, params: &Value) {
        match method {
            "update" => (),
            "scroll_to" => (),
            "available_themes" => (), // TODO
            "available_plugins" => (), // TODO
            "available_languages" => (), // TODO
            "config_changed" => (), // TODO
            "language_changed" => (), // TODO
            _ => println!("unhandled core->fe method {}", method),
        }
    }
}

#[derive(Clone)]
struct AppDispatcher {
    app: Arc<Mutex<Option<App>>>,
}

impl AppDispatcher {
    fn new() -> AppDispatcher {
        AppDispatcher {
            app: Default::default(),
        }
    }

    fn set_app(&self, app: &App) {
        *self.app.lock().unwrap() = Some(app.clone());
    }

    fn set_menu_listeners(&self) {
        let app = self.app.clone();
    }
}


impl Handler for AppDispatcher {
    fn notification(&self, method: &str, params: &Value) {
        if let Some(ref app) = *self.app.lock().unwrap() {
            app.handle_cmd(method, params);
        }
    }
}

#[derive(Debug, Default)]
pub struct Delegate;

impl AppDelegate<ViewState> for Delegate {
    fn command(&mut self, ctx: &mut DelegateCtx, target: Target, cmd: &Command, data: &mut ViewState, env: &Env) -> Handled {
        Handled::Yes
    }
}

pub fn main() {
    setup_log();

    let (xi_peer, rx) = start_xi_thread();

    let main_window = WindowDesc::new(build_root_widget())
        .title("Hello World!")
        .window_size((400.0, 400.0));

    let initial_state: HelloState = HelloState {
        name: "World".into(),
    };

    let handler = AppDispatcher::new();
    handler.set_menu_listeners();

    let core = Core::new(xi_peer, rx, handler.clone());
    let app = App::new(core);


    let launcher = AppLauncher::with_window(main_window);
    let handler = launcher.get_external_handle();

    app.send_notification("client_started", &json!({}));
    app.req_new_view(None);
    app.send_notification("set_theme", &json!({ "theme_name": "InspiredGitHub" }));

    let _thread = thread::spawn(move || {
        handler
            .submit_command(Selector::<()>::new("Test"), Box::new(()), Target::Auto)
            .expect("Failed to send command");
    });

    launcher
        .launch(initial_state)
        .expect("Failed to launch application");
}

fn setup_log() {
    use tracing_subscriber::prelude::*;
    let filter_layer = tracing_subscriber::filter::LevelFilter::DEBUG;
    let fmt_layer = tracing_subscriber::fmt::layer()
        // Display target (eg "my_crate::some_mod::submod") with logs
        .with_target(true);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
