// FIXME: this example is not working

#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use micromath::F32Ext;

use hal::{
    adc::{
        config::{Continuous, Dma as AdcDma, Eoc, SampleTime, Sequence},
        Adc, AdcClaim, ClockSource, Temperature, Vref, DMA as aDMA,
    },
    dma::{config::DmaConfig, stream, stream::DMAExt, transfer, TransferExt},
    gpio::*,
    prelude::*,
    serial::{Event::Rxne, FullConfig, Serial},
    stm32, timer,
};

use core::fmt::Write;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;
    use dwt_systick_monotonic::ExtU32;
    use stm32g4xx_hal::adc::config;
    use stm32g4xx_hal::adc::config::{ExternalTrigger12, SubGroupLength, TriggerMode};
    use stm32g4xx_hal::signature::*;
    use stm32g4xx_hal::timer::{CountDownTimer, Timer, TriggerSource};

    #[shared]
    struct Shared {
        transfer: transfer::CircTransfer<
            stream::Stream0<stm32::DMA1>,
            Adc<stm32::ADC1, aDMA>,
            &'static mut [u16; 15],
        >,
    }

    #[local]
    struct Local {
        serial: Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>,
        timer: CountDownTimer<stm32::TIM1>,
    }

    #[init(local = [buffer: [u16; 15] = [0; 15]])]
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

        adc.set_external_trigger((TriggerMode::RisingEdge, ExternalTrigger12::Tim_1_trgo_2));
        adc.enable_temperature(&ctx.device.ADC12_COMMON);
        adc.enable_vref(&ctx.device.ADC12_COMMON);
        adc.set_continuous(Continuous::Discontinuous);
        adc.set_subgroup_len(SubGroupLength::Three);
        adc.reset_sequence();
        adc.configure_channel(&pa0, Sequence::One, SampleTime::Cycles_640_5);
        adc.configure_channel(&Temperature, Sequence::Two, SampleTime::Cycles_640_5);
        adc.configure_channel(&Vref, Sequence::Three, SampleTime::Cycles_640_5);

        info!("Setup DMA");
        let mut transfer = streams.0.into_circ_peripheral_to_memory_transfer(
            adc.enable_dma(AdcDma::Continuous),
            ctx.local.buffer,
            config,
        );

        transfer.start(|adc| adc.start_conversion());

        info!("Init Timer");
        let mut timer = Timer::new(ctx.device.TIM1, &rcc.clocks);
        timer.set_trigger_source(TriggerSource::Update);
        let mut timer = timer.start_count_down(500.millis());
        timer.listen(timer::Event::TimeOut);

        (
            Shared { transfer },
            Local { serial, timer },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM2, local = [timer])]
    fn tim2(mut ctx: tim2::Context) {
        //info!("time event");
        ctx.local.timer.clear_interrupt(timer::Event::TimeOut);
    }

    #[task(binds = DMA1_CH1, shared = [transfer], local = [serial])]
    fn dma(mut ctx: dma::Context) {
        info!(
            "dma event, data ready to read: {}",
            ctx.shared
                .transfer
                .lock(|transfer| transfer.elements_available())
        );
        if ctx
            .shared
            .transfer
            .lock(|transfer| transfer.elements_available())
            > 3
        {
            // let mut b = [0_u16; 3];
            // let r = ctx
            //     .shared
            //     .transfer
            //     .lock(|transfer| transfer.read_exact(&mut b));
            // info!("read: {}", r);

            // |a0|t|v|a0|t|v|
            //  0  1 2 3  4 5

            // let vdda = VDDA_CALIB * VrefCal::get().read() as u32 / ((b[2] + b[5]) / 2) as u32;
            //
            // info!("vdda: {}mV", vdda);
            //
            // let millivolts =
            //     Vref::sample_to_millivolts_ext((b[0] + b[3]) / 2, vdda, config::Resolution::Twelve);
            // info!("pa0: {}mV", millivolts);
            // let vref =
            //     Vref::sample_to_millivolts_ext((b[2] + b[5]) / 2, vdda, config::Resolution::Twelve);
            // info!("vref: {}mV", vref);
            // let raw_temp = (((b[1] + b[4]) / 2) as f32 * (vdda as f32 / 3000.0)) as u16;
            // let temp = Temperature::temperature_to_degrees_centigrade(raw_temp);
            // info!("temp: {}°C", temp);
            //
            // ctx.local
            //     .serial
            //     .write_fmt(format_args!(
            //         "vdda {}mV, pa0 {}mV, vref {}mV, temp {}.{}°C\r\n",
            //         vdda,
            //         millivolts,
            //         vref,
            //         temp as u16,
            //         (temp.fract() * 100.0) as u16
            //     ))
            //     .unwrap();
        }

        ctx.shared
            .transfer
            .lock(|transfer| transfer.clear_transfer_complete_interrupt());
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        info!("idle");
        loop {
            rtic::export::nop();
        }
    }
}
