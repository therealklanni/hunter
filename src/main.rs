#![feature(vec_remove_item)]
#![feature(trivial_bounds)]
#![feature(try_trait)]
#![allow(dead_code)]

extern crate termion;
extern crate unicode_width;
#[macro_use]
extern crate lazy_static;
extern crate failure;
extern crate failure_derive;
extern crate alphanumeric_sort;
extern crate dirs_2;
extern crate lscolors;
extern crate users;
extern crate chrono;
extern crate rayon;
extern crate libc;
extern crate notify;
extern crate parse_ansi;
extern crate signal_notify;
extern crate tree_magic;
extern crate systemstat;
extern crate image;
extern crate gstreamer;
extern crate gstreamer_app;

extern crate osstrtools;
extern crate pathbuftools;
extern crate async_value;

use failure::Fail;

use std::panic;

mod coordinates;
mod file_browser;
mod files;
mod listview;
mod miller_columns;
mod preview;
mod term;
mod textview;
mod widget;
mod hbox;
mod tabview;
mod fail;
mod minibuffer;
mod proclist;
mod bookmarks;
mod paths;
mod foldview;
mod dirty;
mod fscache;
mod config;
mod stats;
mod icon;
mod imgview;
mod videoview;






use widget::{Widget, WidgetCore};
use term::ScreenExt;
use fail::{HResult, HError};
use file_browser::FileBrowser;
use tabview::TabView;


fn reset_screen(core: &mut WidgetCore) -> HResult<()> {
    core.screen.suspend()
}

fn die_gracefully(core: &WidgetCore) {
    let panic_hook = panic::take_hook();
    let core = core.clone();

    panic::set_hook(Box::new(move |info| {
        let mut core = core.clone();
        reset_screen(&mut core).ok();
        panic_hook(info);
    }));
}

fn main() -> HResult<()> {
    // do this early so it might be ready when needed
    crate::files::load_tags().ok();

    let mut core = WidgetCore::new().expect("Can't create WidgetCore!");

    // Resets terminal when hunter crashes :(
    die_gracefully(&core);

    match run(core.clone()) {
        Ok(_) | Err(HError::Quit) => reset_screen(&mut core),
        Err(err) => {
            reset_screen(&mut core)?;
            eprintln!("{:?}\n{:?}", err, err.cause());
            return Err(err);
        }
    }
}

fn run(mut core: WidgetCore) -> HResult<()> {
    core.screen.clear()?;

    let filebrowser = FileBrowser::new(&core, None)?;
    let mut tabview = TabView::new(&core);
    tabview.push_widget(filebrowser)?;

    tabview.handle_input()?;

    // core.screen.cursor_show()?;
    // core.screen.flush()?;

    Ok(())
}
