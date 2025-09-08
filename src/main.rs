//TODO continue with The Stack

mod emulator;

fn main() {

    //let rom_all: [u8; 0x8010] = *include_bytes!("../Roms/1_Example.nes");
    //let rom_all: [u8; 0x8010] = *include_bytes!("../Roms/2_ReadWrite.nes");
    let rom_all: [u8; 0x8010] = *include_bytes!("../Roms/3_Branches.nes");

    let mut state = emulator::Emulator::new(rom_all);

    while !state.halted {
        state.cpu_cycle();
    }

    println!("a 0x{:x}", state.a);
    println!("x 0x{:x}", state.x);
    println!("y 0x{:x}", state.y);
    println!("$0000 0x{:x}", state.read(0x0000));
    println!("$0001 0x{:x}", state.read(0x0001));
    println!("$0002 0x{:x}", state.read(0x0002));
    println!("$0550 0x{:x}", state.read(0x0550));
}

