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

impl std::cmp::PartialEq for VideoView {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
    }
}

pub struct VideoView {
    core: WidgetCore,
    imgview: Arc<Mutex<ImgView>>,
    raw: DynamicImage,
    player: gstreamer::Element
}

impl VideoView {
    pub fn new_from_file(core: WidgetCore, file: &Path) -> VideoView {

        gstreamer::init().unwrap();

        let source = gstreamer::ElementFactory::make("playbin", None)
        //.ok_or(VideoError::GstreamerCreationError("playbin"))?;
            .unwrap();
        let videorate = gstreamer::ElementFactory::make("videorate", None)
        // .ok_or(VideoError::GstreamerCreationError("videorate"))?;
            .unwrap();
        let pnmenc = gstreamer::ElementFactory::make("pnmenc", None)
        // .ok_or(VideoError::GstreamerCreationError("pnmenc"))?;
            .unwrap();
        let sink = gstreamer::ElementFactory::make("appsink", None)
        // .ok_or(VideoError::GstreamerCreationError("appsink"))?;
            .unwrap();
        let appsink = sink.clone()
            .downcast::<gstreamer_app::AppSink>()
            .unwrap();

        videorate.set_property("max-rate", &(30 as i32)).unwrap();

        let elems = &[&videorate, &pnmenc, &sink];

        let bin = gstreamer::Bin::new(None);
        bin.add_many(elems).unwrap();
        gstreamer::Element::link_many(elems).unwrap();

        // make input for bin point to first element
        let sink = elems[0].get_static_pad("sink").unwrap();
        let ghost = gstreamer::GhostPad::new("sink", &sink)
        // .ok_or(VideoError::GstCreationError("ghost pad"))?;
            .unwrap();
        ghost.set_active(true).unwrap();
        bin.add_pad(&ghost).unwrap();

        let uri = format!("file://{}", &file.to_string_lossy().to_string());

        let imgview = ImgView {
            core: core.clone(),
            buffer: vec![],
            raw: DynamicImage::new_rgb8(0,0)
        };

        let imgview = Arc::new(Mutex::new(imgview));
        let imgview2 = imgview.clone();

        source.set_property("uri", &uri);
        source.set_property("video-sink", &bin.upcast::<gstreamer::Element>()).unwrap();

        appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::new()
                .new_sample({
                    // let stdout = Arc::clone(&stdout);
                    // let zoomer = Arc::clone(&zoomer);
                    move |sink| {
                        dbg!("got sample");
                        let sample = match sink.pull_sample() {
                            Some(sample) => sample,
                            None => return gstreamer::FlowReturn::Eos,
                        };

                        let img = image_from_sample(&sample);
                        img.map(|img| {
                            imgview2.lock().map(|mut view| {
                                view.set_raw_img(img);
                                view.draw();
                            }).ok();
                        });

                        // let mut stdout = stdout.lock().unwrap();
                        // let zoomer = zoomer.lock().unwrap();
                        // match clone.image_from_sample(&sample) {
                        //     Some(mut image) => {
                        //         //clone.display_image(&mut *stdout, &zoomer, &mut image);
                        //         gstreamer::FlowReturn::Ok
                        //     },
                        //     None => gstreamer::FlowReturn::Error
                        // }
                        gstreamer::FlowReturn::Ok
                    }
                })
                .build()
        );

        source.set_state(gstreamer::State::Playing).into_result().unwrap();

        VideoView {
            core: core.clone(),
            imgview: imgview,
            raw: DynamicImage::new_rgb8(0, 0),
            player: source
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

        Ok(())
    }

    fn get_drawlist(&self) -> HResult<String> {
        self.imgview.lock()?.get_drawlist()
    }
}

impl Drop for VideoView {
    fn drop(&mut self) {
        self.player.set_state(gstreamer::State::Null).into_result().unwrap();
    }
}
