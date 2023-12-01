#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::{
    adc::{
        config::{Continuous, Dma as AdcDma, SampleTime, Sequence},
        AdcClaim, ClockSource, Temperature, Vref, Adc, DMA as aDMA,
    },
    gpio::*,
    prelude::*,
    serial::{Event::Rxne, FullConfig, Serial},
    stm32,
    dma::{transfer, stream, config::DmaConfig, stream::DMAExt, TransferExt},
};

use core::fmt::Write;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        transfer: transfer::CircTransfer<stream::Stream0<stm32::DMA1>, Adc<stm32::ADC1, aDMA>, &'static mut [u16; 10]>,
    }

    #[local]
    struct Local {
        serial: Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>,
        //buffer: Option<&'static mut [u16; 2]>,
    }

    #[init(local = [buffer: [u16; 10] = [0; 10]])]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        info!("Init system");

        let mut rcc = ctx.device.RCC.constrain();

        let gpio_a = ctx.device.GPIOA.split(&mut rcc);

        info!("Init UART");

        let tx = gpio_a.pa2.into_alternate();
        let rx = gpio_a.pa3.into_alternate();

        let mut serial = ctx
            .device
            .USART2
            .usart(tx, rx, FullConfig::default(), &mut rcc)
            .unwrap();
        serial.listen(Rxne);

        writeln!(serial, "Hello from USART2\r\n").unwrap();

        info!("Init DMA");
        let streams = ctx.device.DMA1.split(&rcc);
        let config = DmaConfig::default()
            .transfer_complete_interrupt(true)
            .circular_buffer(true)
            .memory_increment(true);

        info!("Init Gpio");
        let pa0 = gpio_a.pa0.into_analog();

        info!("Init Adc1");
        let mut delay = ctx.core.SYST.delay(&rcc.clocks);
        let mut adc = ctx
            .device
            .ADC1
            .claim(ClockSource::SystemClock, &rcc, &mut delay, true);

        adc.enable_temperature(&ctx.device.ADC12_COMMON);
        adc.set_continuous(Continuous::Continuous);
        adc.reset_sequence();
        adc.configure_channel(&pa0, Sequence::One, SampleTime::Cycles_640_5);
        adc.configure_channel(&Temperature, Sequence::Two, SampleTime::Cycles_640_5);

        info!("Setup DMA");
        let mut transfer = streams.0.into_circ_peripheral_to_memory_transfer(
            adc.enable_dma(AdcDma::Continuous),
            ctx.local.buffer,
            config,
        );

        transfer.start(|adc| adc.start_conversion());


        (Shared { transfer }, Local { serial }, init::Monotonics())
    }

    #[task(binds = DMA1_CH1, shared = [transfer], local = [serial])]
    fn dma(mut ctx: dma::Context) {

        let mut b = [0_u16; 4];
        let r = ctx.shared.transfer.lock(|transfer| {
            transfer.read_exact(&mut b)
        }
        );
        info!("read: {}", r);

        let millivolts = Vref::sample_to_millivolts((b[0] + b[2]) / 2);
        info!("pa3: {}mV", millivolts);
        let temp = Temperature::temperature_to_degrees_centigrade((b[1] + b[3]) / 2);
        info!("temp: {}°C", temp); // Note: Temperature seems quite low...
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            rtic::export::nop();
        }
    }
}
