// xcp_lite - rayon demo
// Visualize start and stop of synchronous tasks in worker thread pool
// Taken from the mandelbrot rayon example in the book "Programming Rust" by Jim Blandy and Jason Orendorff

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use image::{ImageBuffer, Rgb};
use num::Complex;
use rayon::prelude::*;
use std::{thread, time::Duration};

use serde::{Deserialize, Serialize};
use xcp::*;
use xcp_type_description_derive::XcpTypeDescription;

const X_RES: usize = 3000;
const Y_RES: usize = 2000;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
struct Mandelbrot {
    x: f64,
    y: f64,
    width: f64,
}

// const MANDELBROT: Mandelbrot = Mandelbrot {
//     x: -1.06,
//     y: 0.271,
//     width: 0.023,
// };

// Complete set
const MANDELBROT: Mandelbrot = Mandelbrot {
    x: -0.5,
    y: 0.0,
    width: 3.0,
};

//---------------------------------------------------------------------------------------
// Image rendering

/// Write the buffer `pixels`, whose dimensions are given by `bounds`, to the file named `filename`.
fn write_image(filename: &str, pixels: &[u8]) -> Result<(), std::io::Error> {
    // Black and white
    // let output = File::create(filename)?;
    // let encoder = PNGEncoder::new(output);
    // encoder.encode(
    //     &pixels,
    //     X_RES as u32,
    //     Y_RES as u32,
    //     ColorType::Gray(8),
    // )?;

    // Rainbox color map (credits to CoPilot)
    let mut color_map = Vec::with_capacity(256);
    for i in 0..256 {
        let (r, g, b) = match i {
            0 => (0, 0, 0),                                              // Black
            1..=42 => (255, (i as f32 * 6.0) as u8, 0),                  // Red to Yellow
            43..=85 => (255 - ((i - 43) as f32 * 6.0) as u8, 255, 0),    // Yellow to Green
            86..=128 => (0, 255, ((i - 86) as f32 * 6.0) as u8),         // Green to Cyan
            129..=171 => (0, 255 - ((i - 129) as f32 * 6.0) as u8, 255), // Cyan to Blue
            172..=214 => (((i - 172) as f32 * 6.0) as u8, 0, 255),       // Blue to Magenta
            215..=255 => (255, 0, 255 - ((i - 215) as f32 * 6.0) as u8), // Magenta to Red
            _ => (0, 0, 0), // Default case (should not be reached)
        };
        let rgb = Rgb::<u8>([r, g, b]);
        color_map.push(rgb);
    }

    // Color
    let mut imgbuf = ImageBuffer::new(X_RES as u32, Y_RES as u32);
    for (x, y, rgb_pixel) in imgbuf.enumerate_pixels_mut() {
        *rgb_pixel = color_map[pixels[y as usize * X_RES as usize + x as usize] as usize];
    }
    imgbuf.save(filename).unwrap();

    Ok(())
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
    pixel: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> Complex<f64> {
    let (width, height) = (
        lower_right.re - upper_left.re,
        upper_left.im - lower_right.im,
    );
    Complex {
        re: upper_left.re + pixel.0 as f64 * width / X_RES as f64,
        im: upper_left.im - pixel.1 as f64 * height / Y_RES as f64,
    }
}

/// Render a line of the Mandelbrot set into a buffer of pixels.
fn render(
    pixels: &mut [u8],
    row: usize,
    length: usize,
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) {
    // Create event for this worker thread and register variable index, which is the upper left corner of the rectangle
    let event = daq_create_event_instance!("task");

    let mut line: u16 = row as u16; // temporary variable to measure the line number as u16
    daq_register_instance!(line, event);
    event.trigger(); // measure line and timestamp of calculation start

    // Render line
    for column in 0..length {
        let point = pixel_to_point((column, row), upper_left, lower_right);
        pixels[column] = match escape_time(point, 255) {
            None => 0,
            Some(count) => 255 - count as u8,
        };
    }

    line = 0; // set to 0 to mark calculation end and measure again to get a timestamp for the end of the calculation
    event.trigger();
    _ = line; // prevent warning about unused variable
}

//---------------------------------------------------------------------------------------

fn main() {
    println!("xcp_lite rayon mandelbrot demo");

    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    const BIND_ADDR: [u8; 4] = [127, 0, 0, 1]; // [192, 168, 0, 83]; // [127, 0, 0, 1];
    XcpBuilder::new("mandelbrot")
        .set_log_level(XcpLogLevel::Info)
        .enable_a2l(true)
        .set_epk("EPK")
        .start_server(XcpTransportLayer::Udp, BIND_ADDR, 5555, 1464)
        .unwrap();

    let mandelbrot = Xcp::create_calseg("mandelbrot", &MANDELBROT, true);

    // The pixel array on stack would overflow the addr offset
    // let mut pixels: [u8; X_RES * Y_RES] = [0; X_RES * Y_RES];
    let mut pixels = vec![0; X_RES * Y_RES];

    // Create event for this worker thread and register variable index, which is the upper left corner of the rectangle
    let event = daq_create_event!("mainloop");
    let mut elapsed_time: f64 = 0.0;
    let mut wait_counter: u32 = 0;
    daq_register!(elapsed_time, event, "duration of image calculation", "s");
    daq_register!(wait_counter, event, "loop counter", "");

    loop {
        thread::sleep(Duration::from_micros(1000));
        let start_time = std::time::Instant::now();

        let lower_right = Complex {
            re: mandelbrot.x + mandelbrot.width / 2.0,
            im: mandelbrot.y - mandelbrot.width / 2.0 * Y_RES as f64 / X_RES as f64,
        };
        let upper_left = Complex {
            re: mandelbrot.x - mandelbrot.width / 2.0,
            im: mandelbrot.y + mandelbrot.width / 2.0 * Y_RES as f64 / X_RES as f64,
        };

        // Calculate image lines in parallel
        let lines: Vec<(usize, &mut [u8])> = pixels.chunks_mut(X_RES).enumerate().collect();
        lines.into_par_iter().for_each(|(y, band)| {
            let band_upper_left = pixel_to_point((0, y), upper_left, lower_right);
            let band_lower_right = pixel_to_point((X_RES, y + 1), upper_left, lower_right);
            render(band, y, X_RES, band_upper_left, band_lower_right);
        });

        // Measure run time
        elapsed_time = start_time.elapsed().as_secs_f64();
        event.trigger();
        println!("Image rendered, duration = {:.3} s", elapsed_time);

        // Measure the pixel array from heap, with an individual event
        //daq_event!(pixels, "pixels");

        // Render image to png
        write_image("mandelbrot.png", &pixels).expect("error writing PNG file");
        println!("Image written to mandelbrot.png");

        // Write A2L file
        Xcp::get().write_a2l();

        // Wait until parameter change and recalculate image
        while !mandelbrot.sync() {
            thread::sleep(Duration::from_millis(100));
            wait_counter += 1;
            event.trigger();
        }
    }

    //Xcp::get().write_a2l();
    //Xcp::stop_server();
}
