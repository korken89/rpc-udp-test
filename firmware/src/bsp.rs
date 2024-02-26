use core::ptr::addr_of_mut;

use embassy_net::{Stack, StackResources};
use embassy_stm32::eth::{Ethernet, PacketQueue};
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::time::Hertz;
use embassy_stm32::{bind_interrupts, eth, peripherals, rng, Config};
use rand_core::RngCore;
use rtic_monotonics::systick::Systick;
use static_cell::StaticCell;

use crate::bsp::ksz8863::KSZ8863SMI;

pub mod ksz8863;

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    RNG => rng::InterruptHandler<peripherals::RNG>;
});

type Device = Ethernet<'static, ETH, KSZ8863SMI>;
pub type NetworkStack = &'static Stack<Device>;

#[inline(always)]
pub fn init(c: cortex_m::Peripherals) -> NetworkStack {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;

        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Bypass,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL168,
            divp: Some(PllPDiv::DIV2), // 8mhz / 4 * 168 / 2 = 168 Mhz.
            divq: None,
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let p = embassy_stm32::init(config);

    #[cfg(feature = "other")]
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    #[cfg(not(feature = "other"))]
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEE];

    static mut PACKETS: PacketQueue<16, 16> = PacketQueue::new();

    let device = Ethernet::new(
        unsafe { &mut *addr_of_mut!(PACKETS) },
        p.ETH,
        Irqs,
        p.PA1,
        p.PA2,
        p.PC1,
        p.PA7,
        p.PC4,
        p.PC5,
        p.PB12,
        p.PB13,
        p.PB11,
        KSZ8863SMI::new(),
        mac_addr,
    );

    let config = embassy_net::Config::dhcpv4(Default::default());
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    //});

    // Generate random seed.
    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    let _ = rng.fill_bytes(&mut seed);
    let seed = u64::from_le_bytes(seed);

    // Init network stack
    static STACK: StaticCell<Stack<Device>> = StaticCell::new();
    static mut RESOURCES: StackResources<4> = StackResources::new();

    let stack = &*STACK.init(Stack::new(
        device,
        config,
        unsafe { &mut *addr_of_mut!(RESOURCES) },
        seed,
    ));

    let systick_token = rtic_monotonics::create_systick_token!();
    Systick::start(c.SYST, 168_000_000, systick_token);
    defmt::info!("init done");

    stack
}
