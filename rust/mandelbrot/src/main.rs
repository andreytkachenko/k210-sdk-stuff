#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![no_std]
#![no_main]

mod palette;

use k210_hal::pac;
use k210_hal::prelude::*;
use k210_hal::stdout::Stdout;
use k210_shared::board::def::{io,DISP_WIDTH,DISP_HEIGHT};
use k210_shared::board::lcd::{LCD,LCDHL,self};
use k210_shared::board::lcd_colors;
use k210_shared::board::lcd_render::render_image;
use k210_shared::soc::fpioa;
use k210_shared::soc::sleep::usleep;
use k210_shared::soc::spi::SPIExt;
use k210_shared::soc::sysctl;
use riscv_rt::entry;

use crate::palette::PALETTE;

/** Connect pins to internal functions */
fn io_mux_init() {
    /* Init SPI IO map and function settings */
    fpioa::set_function(io::LCD_RST.into(), fpioa::function::gpiohs(lcd::RST_GPIONUM));
    fpioa::set_io_pull(io::LCD_RST.into(), fpioa::pull::DOWN); // outputs must be pull-down
    fpioa::set_function(io::LCD_DC.into(), fpioa::function::gpiohs(lcd::DCX_GPIONUM));
    fpioa::set_io_pull(io::LCD_DC.into(), fpioa::pull::DOWN);
    fpioa::set_function(io::LCD_CS.into(), fpioa::function::SPI0_SS3);
    fpioa::set_function(io::LCD_WR.into(), fpioa::function::SPI0_SCLK);

    sysctl::set_spi0_dvp_data(true);
}

/** Set correct voltage for pins */
fn io_set_power() {
    /* Set dvp and spi pin to 1.8V */
    sysctl::set_power_mode(sysctl::power_bank::BANK6, sysctl::io_power_mode::V18);
    sysctl::set_power_mode(sysctl::power_bank::BANK7, sysctl::io_power_mode::V18);
}

fn mandelbrot(cx: f32, cy: f32, iterations: u32) -> u32 {
    let mut z: (f32, f32) = (0.0, 0.0);
    let mut i: u32 = 0;
    while (z.0*z.0 + z.1*z.1) < 2.0*2.0 && i < iterations {
        z = (z.0 * z.0 - z.1 * z.1 + cx, 2.0 * z.0 * z.1 + cy);
        i += 1;
    }
    i
}

#[entry]
fn main() -> ! {
    let p = pac::Peripherals::take().unwrap();

    // Configure clocks (TODO)
    let clocks = k210_hal::clock::Clocks::new();

    usleep(200000);

    // Configure UART
    let serial = p.UARTHS.constrain(115_200.bps(), &clocks);
    let (mut tx, _) = serial.split();

    let mut stdout = Stdout(&mut tx);

    io_mux_init();
    io_set_power();

    let spi = p.SPI0.constrain();
    let mut lcd = LCD::new(spi);
    lcd.init();
    lcd.set_direction(lcd::direction::YX_RLDU);
    lcd.clear(lcd_colors::PURPLE);

    writeln!(stdout, "First frame").unwrap();
    let mut zoom = 5.0f32;
    let ofsx = 0.02997f32;
    let ofsy = 0.80386f32;
    loop {
        render_image(&mut lcd, |x,y| {
            let xx = 2.0 * (x as f32) / ((DISP_WIDTH-1) as f32) - 1.0;
            let yy = 2.0 * (y as f32) / ((DISP_HEIGHT-1) as f32) - 1.0;
            let i = mandelbrot(xx * zoom + ofsx, yy * zoom + ofsy, 20);

            PALETTE[i as usize]
        });

        zoom *= 0.98f32;
    }
}
