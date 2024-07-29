// xcp_lite - rayon demo
// Visualize start and stop of synchronous tasks in worker thread pool
// Taken from the mandelbrot rayon example in the book "Programming Rust" by Jim Blandy and Jason Orendorff

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use image::png::PNGEncoder;
use image::ColorType;
use num::Complex;
use rayon::prelude::*;
use std::fs::File;
use std::{thread, time::Duration};

use xcp::*;

/// Write the buffer `pixels`, whose dimensions are given by `bounds`, to the
/// file named `filename`.
fn write_image(
    filename: &str,
    pixels: &[u8],
    bounds: (usize, usize),
) -> Result<(), std::io::Error> {
    let output = File::create(filename)?;

    let encoder = PNGEncoder::new(output);
    encoder.encode(
        &pixels,
        bounds.0 as u32,
        bounds.1 as u32,
        ColorType::Gray(8),
    )?;

    Ok(())
}

//---------------------------------------------------------------------------------------

/// Try to determine if `c` is in the Mandelbrot set, using at most `limit`
/// iterations to decide.
///
/// If `c` is not a member, return `Some(i)`, where `i` is the number of
/// iterations it took for `c` to leave the circle of radius two centered on the
/// origin. If `c` seems to be a member (more precisely, if we reached the
/// iteration limit without being able to prove that `c` is not a member),
/// return `None`.
fn escape_time(c: Complex<f64>, limit: usize) -> Option<usize> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    for i in 0..limit {
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
        z = z * z + c;
    }

    None
}

/// Given the row and column of a pixel in the output image, return the
/// corresponding point on the complex plane.
///
/// `bounds` is a pair giving the width and height of the image in pixels.
/// `pixel` is a (column, row) pair indicating a particular pixel in that image.
/// The `upper_left` and `lower_right` parameters are points on the complex
/// plane designating the area our image covers.
fn pixel_to_point(
    bounds: (usize, usize),
    pixel: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> Complex<f64> {
    let (width, height) = (
        lower_right.re - upper_left.re,
        upper_left.im - lower_right.im,
    );
    Complex {
        re: upper_left.re + pixel.0 as f64 * width / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64, // Why subtraction here? pixel.1 increases as we go down,
                                                                       // but the imaginary component increases as we go up.
    }
}

/// Render a rectangle of the Mandelbrot set into a buffer of pixels.
///
/// The `bounds` argument gives the width and height of the buffer `pixels`,
/// which holds one grayscale pixel per byte. The `upper_left` and `lower_right`
/// arguments specify points on the complex plane corresponding to the upper-
/// left and lower-right corners of the pixel buffer.
fn render(
    pixels: &mut [u8],
    row: usize,
    length: usize,
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) {
    // Create event for this worker thread and register variable index, which is the upper left corner of the rectangle
    let event = daq_create_event_instance!("task");
    let mut line: u16 = row as u16;
    daq_register_instance!(line, event);
    event.trigger();

    for column in 0..length {
        let point = pixel_to_point((length, 1), (column, row), upper_left, lower_right);
        pixels[column] = match escape_time(point, 255) {
            None => 0,
            Some(count) => 255 - count as u8,
        };
    }

    // Done
    line = 0;
    event.trigger();
}

//---------------------------------------------------------------------------------------

fn main() {
    println!("xcp_lite_rayon_demo");

    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    const BIND_ADDR: [u8; 4] = [192, 168, 0, 83]; // [127, 0, 0, 1]
    XcpBuilder::new("rayon_demo")
        .set_log_level(XcpLogLevel::Info)
        .enable_a2l(true)
        .set_epk("EPK")
        .start_server(XcpTransportLayer::Udp, BIND_ADDR, 5555, 1464)
        .unwrap();

    loop {
        thread::sleep(Duration::from_micros(1000));

        const X_RES: usize = 3000;
        const Y_RES: usize = 2000;

        let upper_left = Complex { re: -1.2, im: 0.35 };
        let lower_right = Complex { re: -1.0, im: 0.25 };

        let mut pixels = vec![0; X_RES * Y_RES];

        // Scope of slicing up `pixels` into horizontal lines.
        {
            let lines: Vec<(usize, &mut [u8])> = pixels.chunks_mut(X_RES).enumerate().collect();

            lines.into_par_iter().for_each(|(line, band)| {
                let band_upper_left =
                    pixel_to_point((X_RES, Y_RES), (0, line), upper_left, lower_right);
                let band_lower_right =
                    pixel_to_point((X_RES, Y_RES), (X_RES, line + 1), upper_left, lower_right);
                render(band, line, X_RES, band_upper_left, band_lower_right);
            });
        }

        write_image("mandel.png", &pixels, (X_RES, Y_RES)).expect("error writing PNG file");
        println!("Image written to mandel.png");
    }

    //Xcp::get().write_a2l();
    //Xcp::stop_server();
}
