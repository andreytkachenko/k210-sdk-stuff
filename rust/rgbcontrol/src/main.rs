#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![no_std]
#![no_main]

use k210_hal::pac;
use k210_hal::prelude::*;
use k210_hal::stdout::Stdout;
use k210_shared::board::def::{
    io, DISP_HEIGHT, DISP_WIDTH, NS2009_ADDR_BITS, NS2009_CAL, NS2009_CLK, NS2009_SLV_ADDR,
};
use k210_shared::board::lcd::{self, LCD, LCDHL};
use k210_shared::board::lcd_colors::{clampf, hsv2rgb, rgbf565};
use k210_shared::board::lcd_render::render_image;
use k210_shared::board::ns2009::TouchScreen;
use k210_shared::soc::fpioa;
use k210_shared::soc::i2c::{I2CExt, I2C};
use k210_shared::soc::pwm::{pwm_set, pwm_start, Channel};
use k210_shared::soc::sleep::usleep;
use k210_shared::soc::spi::SPIExt;
use k210_shared::soc::sysctl;
use riscv_rt::entry;

// Factor for equalizing relative brightness of R/G/B leds
const R_SCALE: f32 = 1.0;
const G_SCALE: f32 = 0.16;
const B_SCALE: f32 = 0.33;

fn io_init() {
    /* Init SPI IO map and function settings */
    fpioa::set_function(
        io::LCD_RST.into(),
        fpioa::function::gpiohs(lcd::RST_GPIONUM),
    );
    fpioa::set_io_pull(io::LCD_RST.into(), fpioa::pull::DOWN); // outputs must be pull-down
    fpioa::set_function(io::LCD_DC.into(), fpioa::function::gpiohs(lcd::DCX_GPIONUM));
    fpioa::set_io_pull(io::LCD_DC.into(), fpioa::pull::DOWN);
    fpioa::set_function(io::LCD_CS.into(), fpioa::function::SPI0_SS3);
    fpioa::set_function(io::LCD_WR.into(), fpioa::function::SPI0_SCLK);

    /* Route PWM outputs of TIMER0 to RGB leds */
    fpioa::set_function(io::LED_R as u8, fpioa::function::TIMER0_TOGGLE1);
    fpioa::set_function(io::LED_G as u8, fpioa::function::TIMER0_TOGGLE2);
    fpioa::set_function(io::LED_B as u8, fpioa::function::TIMER0_TOGGLE3);

    sysctl::set_spi0_dvp_data(true);

    /* I2C0 for touch-screen */
    fpioa::set_function(io::I2C1_SCL.into(), fpioa::function::I2C0_SCLK);
    fpioa::set_function(io::I2C1_SDA.into(), fpioa::function::I2C0_SDA);

    /* Set DVP and SPI pins to 1.8V */
    sysctl::set_power_mode(sysctl::power_bank::BANK6, sysctl::io_power_mode::V18);
    sysctl::set_power_mode(sysctl::power_bank::BANK7, sysctl::io_power_mode::V18);
}

/** Color picker */
fn color_from_xy(x: u16, y: u16, v: f32) -> (f32, f32, f32) {
    hsv2rgb(
        360.0 * (x as f32) / (DISP_WIDTH as f32),
        (y as f32) / ((DISP_HEIGHT - 1) as f32),
        v,
    )
}

#[entry]
fn main() -> ! {
    let p = pac::Peripherals::take().unwrap();
    let clocks = k210_hal::clock::Clocks::new();

    usleep(200000);

    let serial = p.UARTHS.constrain(115_200.bps(), &clocks);
    let (mut tx, _) = serial.split();
    let mut stdout = Stdout(&mut tx);

    io_init();

    writeln!(stdout, "NS2009 init").unwrap();
    let i2c = p.I2C0.constrain();
    i2c.init(NS2009_SLV_ADDR, NS2009_ADDR_BITS, NS2009_CLK);
    let mut ts = if let Some(ts) = TouchScreen::init(i2c, NS2009_CAL) {
        ts
    } else {
        writeln!(stdout, "NS2009 init failure").unwrap();
        panic!("Fatal error");
    };

    writeln!(stdout, "LCD init").unwrap();
    let spi = p.SPI0.constrain();
    let mut lcd = LCD::new(spi);
    lcd.init();
    lcd.set_direction(lcd::direction::YX_LRUD);
    render_image(&mut lcd, |x, y| {
        let (r, g, b) = color_from_xy(x, y, 1.0);
        rgbf565(r, g, b)
    });

    writeln!(stdout, "start PWM").unwrap();
    sysctl::clock_enable(sysctl::clock::TIMER0);
    pwm_start(Channel::CH1);
    pwm_start(Channel::CH2);
    pwm_start(Channel::CH3);

    let freq = 10000;

    loop {
        if let Some(ev) = ts.poll() {
            if ev.z > 0
                && ev.x >= 0
                && ev.x < (DISP_WIDTH as i32)
                && ev.y >= 0
                && ev.y < (DISP_HEIGHT as i32)
            {
                //writeln!(stdout, "{:?}", ev).unwrap();
                let (r, g, b) =
                    color_from_xy(ev.x as u16, ev.y as u16, clampf(ev.z as f32 / 1000.0));

                pwm_set(Channel::CH1, freq, 1.0 - r * R_SCALE);
                pwm_set(Channel::CH2, freq, 1.0 - g * G_SCALE);
                pwm_set(Channel::CH3, freq, 1.0 - b * B_SCALE);
            }
        }

        usleep(10000);
    }
}
