use image;
use image::Pixel;
use image::FilterType;
use image::DynamicImage;
use image::GenericImageView;

use termion::color::{Bg, Fg, Rgb};

use gstreamer::{self, prelude::*};
use gstreamer_app;

use crate::widget::{Widget, WidgetCore};
use crate::fail::{HResult, ErrorLog};
use crate::imgview::ImgView;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::io::BufRead;

impl std::cmp::PartialEq for VideoView {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
    }
}

pub struct VideoView {
    core: WidgetCore,
    buffer: String,
    //imgview: ImgView,
    //raw: DynamicImage,
    // player: Child,
    frame_receiver: Receiver<String>,
}

impl VideoView {
    pub fn new_from_file(core: WidgetCore, file: &Path) -> VideoView {
        let (tx, rx) = channel();
        let corecl = core.clone();
        let file = file.to_path_buf();
        std::thread::spawn(move || {
            let core = corecl;
            let (xsize, ysize) = core.coordinates.size_u();

            let mut player = std::process::Command::new("termplay")
                .arg("-q")
                .arg("-w")
                .arg(format!("{}", xsize+1))
                .arg("-h")
                .arg(format!("{}", ysize*2))
                .arg(file.to_string_lossy().to_string())
                .stdin(Stdio::inherit())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn();

            let stdout = player.unwrap().stdout.unwrap();
            let stdout = std::io::BufReader::new(stdout);

            let mut frame = String::new();
            for line in stdout.lines() {

                if line.as_ref().ok() == Some(String::from("\x1b[0m")).as_ref() {
                    dbg!("line");
                    let full_frame = frame.clone();

                    let mut img = ImgView {
                        core: core.clone(),
                        buffer: full_frame.lines().map(|l| l.to_string()).collect()
                    };
                    img.draw();

                    // tx.send(full_frame);
                    // core.get_sender().send(crate::widget::Events::WidgetReady);
                    frame.clear();
                } else {
                    if let Ok(line) = line {
                        frame += &line;
                    }
                }
            }
        });

        VideoView {
            core: core.clone(),
            buffer: String::new(),
            //imgview: imgview,
            //raw: DynamicImage::new_rgb8(0, 0),
            // player: source
            frame_receiver: rx
        }
    }
}


fn image_from_sample(sample: &gstreamer::sample::SampleRef) -> Option<DynamicImage> {
        let buffer = sample.get_buffer()?;
        let map = buffer.map_readable()?;
        image::load_from_memory_with_format(&map, image::ImageFormat::PNM).ok()
}


impl Widget for VideoView {
    fn get_core(&self) -> HResult<&WidgetCore> {
        Ok(&self.core)
    }

    fn get_core_mut(&mut self) -> HResult<&mut WidgetCore> {
        Ok(&mut self.core)
    }

    fn refresh(&mut self) -> HResult<()> {
        dbg!("refresh");
        // let frame = self.frame_receiver.recv()?;

        // self.buffer = frame;
        Ok(())
    }

    fn get_drawlist(&self) -> HResult<String> {
        let img = ImgView {
            core: self.core.clone(),
            buffer: self.buffer.lines().map(|l| l.to_string()).collect()
        };
        img.get_drawlist()
    }
}

impl Drop for VideoView {
    fn drop(&mut self) {
        //self.player.set_state(gstreamer::State::Null).into_result().unwrap();
    }
}
