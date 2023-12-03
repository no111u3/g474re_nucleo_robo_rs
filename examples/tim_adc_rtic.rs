// FIXME: this example is not working

#![no_std]
#![no_main]

use panic_halt as _;
use rtic;
use stm32g4xx_hal as hal;

use defmt::info;
use defmt_rtt as _;

use hal::{
    adc::{
        config::{Continuous, Eoc, ExternalTrigger12, SampleTime, Sequence, TriggerMode},
        Active, Adc, AdcClaim, ClockSource, Temperature, Vref,
    },
    gpio::*,
    prelude::*,
    serial::{Event::Rxne, FullConfig, Serial},
    stm32,
};

use core::fmt::Write;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;
    use dwt_systick_monotonic::ExtU32;

    use stm32g4xx_hal::timer::{Timer, TriggerSource};

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        serial: Serial<stm32::USART2, gpioa::PA2<Alternate<7>>, gpioa::PA3<Alternate<7>>>,
        adc: Adc<stm32::ADC1, Active>,
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
        adc.reset_sequence();
        adc.configure_channel(&pa0, Sequence::One, SampleTime::Cycles_640_5);
        adc.configure_channel(&Temperature, Sequence::Two, SampleTime::Cycles_640_5);
        adc.configure_channel(&Vref, Sequence::Three, SampleTime::Cycles_640_5);
        adc.set_end_of_conversion_interrupt(Eoc::Sequence);

        let adc = adc.enable();
        let adc = adc.start_conversion();

        info!("Init Timer");
        let mut timer = Timer::new(ctx.device.TIM1, &rcc.clocks);
        timer.set_trigger_source(TriggerSource::Update);
        timer.start_count_down(500.millis());

        (Shared {}, Local { serial, adc }, init::Monotonics())
    }

    #[task(binds=ADC1_2, local = [adc])]
    fn adc(ctx: adc::Context) {
        info!("adc");
        info!("Read for adc: {}", ctx.local.adc.current_sample());
        ctx.local.adc.clear_end_conversion_flag();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        info!("idle");
        loop {
            rtic::export::nop();
        }
    }
}
