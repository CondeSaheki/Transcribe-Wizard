use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext, ContentFormat};
use image::DynamicImage;
use imgui::Context;
use imgui_glow_renderer::{
    glow::{self, HasContext},
    AutoRenderer,
};
use imgui_sdl2_support::SdlPlatform;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
#[allow(unused)]
use rten_tensor::prelude::*;
use sdl2::{
    event::Event,
    video::{GLProfile, Window},
};
use std::error::Error;

// Convert an image to a string using OCRengine
fn image_to_str(engine: &OcrEngine, image: &DynamicImage) -> Result<String, Box<dyn Error>> {
    let image_rgb = image.to_rgb8();
    let image_source = ImageSource::from_bytes(image_rgb.as_raw(), image_rgb.dimensions())?;
    let ocr_input = engine.prepare_input(image_source)?;
    let word_rects = engine.detect_words(&ocr_input)?;
    let line_rects = engine.find_text_lines(&ocr_input, &word_rects);
    let line_texts = engine.recognize_text(&ocr_input, &line_rects)?;

    Ok(line_texts
        .into_iter()
        .flatten()
        .filter(|line| line.to_string().len() > 1)
        .map(|line| line.to_string())
        .collect::<Vec<String>>()
        .join(" "))
}

// get and convert content from clipboard
fn clipboard_str(
    engine: &OcrEngine,
    clipboard_context: &ClipboardContext,
) -> Result<String, Box<dyn std::error::Error>> {
    if clipboard_context.has(ContentFormat::Text) {
        match clipboard_context.get_text() {
            Ok(text) => return Ok(text),
            Err(err) => return Err(format!("Failed to get text from clipboard: {}", err).into()),
        }
    }

    if clipboard_context.has(ContentFormat::Image) {
        let image_data = match clipboard_context.get_image() {
            Ok(image) => image,
            Err(err) => return Err(format!("Failed to get image from clipboard: {}", err).into()),
        };
        let image = match image_data.get_dynamic_image() {
            Ok(image) => image,
            Err(err) => {
                return Err(
                    format!("Failed to convert image data to dynamic image: {}", err).into(),
                )
            }
        };
        match image_to_str(engine, &image) {
            Ok(text) => return Ok(text),
            Err(err) => return Err(format!("Failed to extract text from image: {}", err).into()),
        }
    }

    Err("Unhandled clipboard content: neither text nor image".into())
}

// Create a new glow context.
fn glow_context(window: &Window) -> glow::Context {
    unsafe {
        glow::Context::from_loader_function(|s| window.subsystem().gl_get_proc_address(s) as _)
    }
}

fn main() {
    /* initialize SDL and its video subsystem */
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    /* hint SDL to initialize an OpenGL 3.3 core profile context */
    let gl_attr = video_subsystem.gl_attr();

    gl_attr.set_context_version(3, 3);
    gl_attr.set_context_profile(GLProfile::Core);

    /* create a new window, be sure to call opengl method on the builder when using glow! */
    let window = video_subsystem
        .window("Hello imgui-rs!", 1280, 720)
        .allow_highdpi()
        .opengl()
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    /* create a new OpenGL context and make it current */
    let gl_context = window.gl_create_context().unwrap();
    window.gl_make_current(&gl_context).unwrap();

    /* enable vsync to cap framerate */
    window.subsystem().gl_set_swap_interval(1).unwrap();

    /* create new glow and imgui contexts */
    let gl = glow_context(&window);

    /* create context */
    let mut imgui = Context::create();

    /* disable creation of files on disc */
    imgui.set_ini_filename(None);
    imgui.set_log_filename(None);

    /* setup platform and renderer, and fonts to imgui */
    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

    /* create platform and renderer */
    let mut platform = SdlPlatform::new(&mut imgui);
    let mut renderer = AutoRenderer::new(gl, &mut imgui).unwrap();

    // /* setup OCR context */
    // let detection_model_path = file_path("src/text-detection.rten");
    // let rec_model_path = file_path("src/text-recognition.rten");

    let detection_model = match Model::load_file("text-detection.rten") {
        Ok(model) => model,
        Err(err) => {
            eprintln!("Error loading detection model: {}", err);
            std::process::exit(1)
        }
    };
    let recognition_model = match Model::load_file("text-recognition.rten") {
        Ok(model) => model,
        Err(err) => {
            eprintln!("Error loading recognition model: {}", err);
            std::process::exit(1)
        }
    };

    let ocr = match OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    }) {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("Error creating OCR engine: {}", err);
            std::process::exit(1)
        }
    };

    /* setup clipboard context */
    let clipboard = ClipboardContext::new().unwrap();

    let mut text = String::new();

    /* start main loop */
    let mut event_pump = sdl.event_pump().unwrap();

    'main: loop {
        for event in event_pump.poll_iter() {
            /* pass all events to imgui platfrom */
            platform.handle_event(&mut imgui, &event);

            if let Event::Quit { .. } = event {
                break 'main;
            }
        }

        /* call prepare_frame before calling imgui.new_frame() */
        platform.prepare_frame(&mut imgui, &window, &event_pump);

        let ui = imgui.new_frame();

        /* create imgui UI here */

        if ui.button("Get clipboard") {
            text = match clipboard_str(&ocr, &clipboard) {
                Ok(text) => text,
                Err(err) => format!("Error getting text from clipboard: {}", err),
            }
        }

        ui.same_line();

        if ui.button("Copy") {
            match clipboard.set_text(text.clone()) {
                Ok(()) => (),
                Err(err) => {
                    text = format!("Error setting text to clipboard: {}", err);
                }
            }
        }

        ui.text(text.as_str());

        /* render */
        let draw_data = imgui.render();

        unsafe { renderer.gl_context().clear(glow::COLOR_BUFFER_BIT) };
        renderer.render(draw_data).unwrap();

        window.gl_swap_window();
    }
}
