// xcp-lite - rayon demo
// Visualize start and stop of synchronous tasks in worker thread pool
// Inspired by the mandelbrot rayon example in the book "Programming Rust" by Jim Blandy and Jason Orendorff

// cargo r --example rayon_demo
// Creates madelbrot.a2l and mandelbrot.png in current directory

use anyhow::Result;
use image::{ImageBuffer, Rgb};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use num::Complex;
use rayon::prelude::*;
use std::{thread, time::Duration};
use xcp::*;
use xcp_type_description::prelude::*;

//---------------------------------------------------------------------------------------
// Calibratable parameters

const IMAGE_FILE_NAME: &str = "mandelbrot.png";
const IMAGE_SIZE: usize = 8;
const X_RES: usize = 1024 * IMAGE_SIZE;
const Y_RES: usize = 768 * IMAGE_SIZE;

#[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
struct Mandelbrot {
    x: f64, // Center of the set area to render
    y: f64,
    width: f64, // Width of the set area to render
}

// Defaults
//const MANDELBROT: Mandelbrot = Mandelbrot { x: -0.5, y: 0.0, width: 3.0 }; // Complete set
//const MANDELBROT: Mandelbrot = Mandelbrot { x: -1.4, y: 0.0, width: 0.015 };
const MANDELBROT: Mandelbrot = Mandelbrot { x: -0.8015, y: 0.1561, width: 0.0055 };

//---------------------------------------------------------------------------------------
// Coloring

// Normalizes color intensity values within RGB range
fn normalize(color: f32, factor: f32) -> u8 {
    ((color * factor).powf(0.8) * 255.) as u8
}

// Function converting intensity values to RGB
fn wavelength_to_rgb(i: u32) -> Rgb<u8> {
    let wave = i as f32;

    let (r, g, b) = match i {
        380..=439 => ((440. - wave) / (440. - 380.), 0.0, 1.0),
        440..=489 => (0.0, (wave - 440.) / (490. - 440.), 1.0),
        490..=509 => (0.0, 1.0, (510. - wave) / (510. - 490.)),
        510..=579 => ((wave - 510.) / (580. - 510.), 1.0, 0.0),
        580..=644 => (1.0, (645. - wave) / (645. - 580.), 0.0),
        645..=780 => (1.0, 0.0, 0.0),
        _ => (0.0, 0.0, 0.0),
    };

    let factor = match i {
        380..=419 => 0.3 + 0.7 * (wave - 380.) / (420. - 380.),
        701..=780 => 0.3 + 0.7 * (780. - wave) / (780. - 700.),
        _ => 1.0,
    };

    let (r, g, b) = (normalize(r, factor), normalize(g, factor), normalize(b, factor));
    Rgb::from([r, g, b])
}

fn get_color_mag() -> Vec<Rgb<u8>> {
    // Map iterations to colors
    let mut color_map = Vec::with_capacity(256);
    for i in 0..256 {
        let rgb = wavelength_to_rgb(379 + (i * (781 - 379)) / 255);

        color_map.push(rgb);
    }
    color_map
}

//---------------------------------------------------------------------------------------
// Image rendering

// Write the buffer `pixels` to the file named `filename`.
fn write_image(filename: &str, pixels: &[u8]) {
    let color_map = get_color_mag();

    // Create rgb image buffer and write to file
    let mut imgbuf = ImageBuffer::new(X_RES as u32, Y_RES as u32);
    for (x, y, rgb_pixel) in imgbuf.enumerate_pixels_mut() {
        *rgb_pixel = color_map[pixels[y as usize * X_RES + x as usize] as usize];
    }
    imgbuf.save(filename).unwrap();
}

//---------------------------------------------------------------------------------------
// Mandelbrot set

/// Try to determine if `c` is in the Mandelbrot set, using at most `limit`
/// iterations to decide.
///
/// If `c` is not a member, return `Some(i)`, where `i` is the number of
/// iterations it took for `c` to leave the circle of radius two centered on the
/// origin. If `c` seems to be a member (more precisely, if we reached the
/// iteration limit without being able to prove that `c` is not a member),
/// return `None`.
fn mandelbrot(c: Complex<f64>, limit: usize) -> Option<usize> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    for i in 0..limit {
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
        z = z * z + c;
    }

    None
}

//---------------------------------------------------------------------------------------

/// Given the row and column of a pixel in the output image, return the
/// corresponding point on the complex plane.
///
/// `bounds` is a pair giving the width and height of the image in pixels.
/// `pixel` is a (column, row) pair indicating a particular pixel in that image.
/// The `upper_left` and `lower_right` parameters are points on the complex
/// plane designating the area our image covers.
fn pixel_to_point(pixel: (usize, usize), upper_left: Complex<f64>, lower_right: Complex<f64>) -> Complex<f64> {
    let (width, height) = (lower_right.re - upper_left.re, upper_left.im - lower_right.im);
    Complex {
        re: upper_left.re + pixel.0 as f64 * width / X_RES as f64,
        im: upper_left.im - pixel.1 as f64 * height / Y_RES as f64,
    }
}

/// Render a line of the Mandelbrot set into a buffer of pixels.
fn render(pixels: &mut [u8], row: usize, upper_left: Complex<f64>, lower_right: Complex<f64>) {
    // Create event for this worker thread and register variable index, which is the upper left corner of the rectangle
    let event = daq_create_event_tli!("task");

    let mut line: u16 = row as u16; // temporary variable to measure the line number as u16
    daq_register_tli!(line, event);
    event.trigger(); // measure line and timestamp of calculation start

    // Render line
    for (column, pixel) in pixels.iter_mut().enumerate() {
        let point = pixel_to_point((column, row), upper_left, lower_right);
        *pixel = mandelbrot(point, 254).unwrap_or(255) as u8;
    }

    line = 0; // set to 0 to mark calculation end and measure again to get a timestamp for the end of the calculation
    event.trigger();
    _ = line; // prevent warning about unused variable line
}

//---------------------------------------------------------------------------------------

fn main() -> Result<()> {
    println!("xcp-lite rayon mandelbrot demo");

    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();

    info!("Number of logical cores is {}", num_cpus::get());

    const BIND_ADDR: [u8; 4] = [127, 0, 0, 1];

    let xcp = XcpBuilder::new("mandelbrot")
        .set_log_level(XcpLogLevel::Debug)
        .set_epk("EPK")
        .start_server(XcpTransportLayer::Udp, BIND_ADDR, 5555)?;

    let mandelbrot = xcp.create_calseg("mandelbrot", &MANDELBROT);
    mandelbrot.register_fields();

    // The pixel array on heap
    let mut pixels = vec![0; X_RES * Y_RES];

    // Create event for this worker thread and register variable index, which is the upper left corner of the rectangle
    let event_mainloop = daq_create_event!("mainloop");
    let event_update = daq_create_event!("update");
    let mut elapsed_time: f64 = 0.0;
    let mut mainloop_counter: u32 = 0;
    let mut update_counter: u32 = 0;
    daq_register!(elapsed_time, event_update, "calculation duration", "s");
    daq_register!(mainloop_counter, event_mainloop, "mainloop counter", "");
    daq_register!(update_counter, event_update, "update counter", "");

    // Recalculate image in a loop with 10 ms pause
    let mut first = true;
    loop {
        thread::sleep(Duration::from_micros(1000)); // 1ms
        mainloop_counter += 1;
        event_mainloop.trigger();

        // On first iteration or after parameter changes: render image and write to file
        if first || mandelbrot.sync() {
            {
                let start_time = std::time::Instant::now();

                // Calculate image lines in parallel
                let lower_right = Complex {
                    re: mandelbrot.x + mandelbrot.width / 2.0,
                    im: mandelbrot.y - mandelbrot.width / 2.0 * Y_RES as f64 / X_RES as f64,
                };
                let upper_left = Complex {
                    re: mandelbrot.x - mandelbrot.width / 2.0,
                    im: mandelbrot.y + mandelbrot.width / 2.0 * Y_RES as f64 / X_RES as f64,
                };
                let lines: Vec<(usize, &mut [u8])> = pixels.chunks_mut(X_RES).enumerate().collect();
                lines.into_par_iter().for_each(|(y, band)| {
                    let band_upper_left = pixel_to_point((0, y), upper_left, lower_right);
                    let band_lower_right = pixel_to_point((X_RES, y + 1), upper_left, lower_right);
                    render(band, y, band_upper_left, band_lower_right);
                });

                elapsed_time = start_time.elapsed().as_secs_f64();

                // Measure the pixel array from heap, with an individual event
                // daq_event_for_ref!(
                //     pixels,
                //     RegistryDataType::Ubyte,
                //     X_RES as u16,
                //     Y_RES as u16,
                //     "pixel array"
                // );
            }

            // Write image to file
            write_image(IMAGE_FILE_NAME, &pixels);
            println!(
                "Image written to {}, resolution {}x{} {:.1}MB, duration={:.4}s",
                IMAGE_FILE_NAME,
                X_RES,
                Y_RES,
                (X_RES * Y_RES * 3) as f64 / 1000000.0,
                elapsed_time
            );
            update_counter += 1;
            event_update.trigger();
        }

        first = false;
    }
}
