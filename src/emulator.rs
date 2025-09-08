
pub struct Flags {
    flag_carry: bool,
    flag_zero: bool,
    flag_interrupt_disable: bool,
    flag_decimal: bool,
    flag_overflow: bool,
    flag_negative: bool,
}

pub struct Emulator {
    ram: [u8; 0x800],
    rom_header: [u8; 16],
    rom: [u8; 0x8000],
    program_counter: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub halted: bool,
    pub cycle: u16,
    opcode: u8,
    temp_bytes: [u8; 4], // Can be used to store values inbetween cpu cycles, not to be confused with registers
    flags: Flags
}

impl Emulator {
    pub fn new(rom_all: [u8; 0x8010]) -> Self {
        let mut ret = Self { ram: [0; 0x800],
            rom_header: rom_all[0..0x10].try_into().expect("Error reading ROM header"),
            rom: rom_all[0x10..0x8010].try_into().expect("Error reading ROM contents"),
            program_counter: 0,
            a: 0,
            x: 0,
            y: 0,
            halted: false, 
            cycle: 0, 
            opcode: 0, 
            temp_bytes: [0; 4],
            flags: Flags { flag_carry: false, flag_zero: false, flag_interrupt_disable: true, flag_decimal: false, flag_overflow: false, flag_negative: false }
        };

        let pcl = ret.read(0xFFFC);
        let pch = ret.read(0xFFFD);
        let reset_vector = ((pch as u16) * 0x100) + pcl as u16;
        ret.program_counter = reset_vector;

        return ret;
    }

    pub fn read(&self, address: u16) -> u8 {
        if address < 0x8000 {
            return self.ram[(address % 0x800) as usize];
        }
        if address >= 0x8000 {
            return self.rom[(address-0x8000) as usize];
        }

        //TODO should probably return an error instead
        return 0;
    }

    pub fn write(&mut self, address: u16, value: u8) {
        if address < 0x8000 {
            self.ram[(address % 0x800) as usize] = value;
        }
    }

    //Counting up the cycles will allow instructions between cycles which may come in useful in the future
    // That would not be possible if all cycles of an instruction were performed immediatly, though that would certanly make it more readable
    pub fn cpu_cycle(&mut self) {
        //By default, the cpu cycle count is reset to 0 after an instruction
        let mut reset = true;

        if self.cycle == 0 {
            self.opcode = self.read(self.program_counter);
            reset = false;
            self.increment_pc();
        }
        else if self.cycle == 1 {
            match self.opcode {
                0x02 => self.halted = true,
                0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0 //BPL, BMI, BVC, BVS, BCC, BCS, BNE, BEQ
                 | 0x84..0x86 //STY, STA, STX Zero page cycle 2
                 | 0x8C..0x8E //STY, STA, STX Absolute cycle 2
                 | 0xA5 //LDA Zero page cycle 2
                 | 0xAD //LDA Absolute cycle 2
                 => { self.temp_bytes[0] = self.read(self.program_counter); reset = false }, 
                0xA0 => { self.y = self.read(self.program_counter); }, //LDY Immediate
                0xA2 => { self.x = self.read(self.program_counter); }, //LDX Immediate
                0xA9 => { self.a = self.read(self.program_counter); self.set_flags(self.a) }, //LDA Immediate
                _ => {self.missing_opcode();}
            };

            self.increment_pc();
        }
        else if self.cycle == 2 {
            match self.opcode {
                0x10 => { if !self.flags.flag_negative { self.take_branch(self.temp_bytes[0]) } ; }, //BPL
                0x30 => { if self.flags.flag_negative { self.take_branch(self.temp_bytes[0]) } ; }, //BMI
                0x84 => { self.write(self.temp_bytes[0] as u16, self.y); }, //STY Zero page cycle 3
                0x85 => { self.write(self.temp_bytes[0] as u16, self.a); }, //STA Zero page cycle 3
                0x86 => { self.write(self.temp_bytes[0] as u16, self.x); }, //STX Zero page cycle 3
                0x8C..0x8E //STY, STA, STX Absolute cycle 3
                | 0xAD => //LDA Absolute cycle 3
                { self.temp_bytes[1] = self.read(self.program_counter); reset = false; self.increment_pc(); }, 
                0xA5 => { self.a = self.read(self.temp_bytes[0] as u16); self.set_flags(self.a) }, //LDA Zero page cycle 3
                0xD0 => { if !self.flags.flag_zero { self.take_branch(self.temp_bytes[0]) } ; }, //BNE
                0xF0 => { if self.flags.flag_zero { self.take_branch(self.temp_bytes[0]) } ; }, //BEQ
                _ => {self.missing_opcode();}
            };

        }
        else if self.cycle == 3 {
            match self.opcode {
                0x8C => { self.write((self.temp_bytes[1] as u16) * 0x100 + self.temp_bytes[0] as u16, self.y); }, //STY Absolute cycle 4
                0x8D => { self.write((self.temp_bytes[1] as u16) * 0x100 + self.temp_bytes[0] as u16, self.a); }, //STA Absolute cycle 4
                0x8E => { self.write((self.temp_bytes[1] as u16) * 0x100 + self.temp_bytes[0] as u16, self.x); }, //STX Absolute cycle 4
                0xAD => { self.a = self.read((self.temp_bytes[1] as u16) * 0x100 + self.temp_bytes[0] as u16); self.set_flags(self.a)}, //LDA Absolute cycle 4
                _ => {self.missing_opcode();}
            };
        }

        //TODO this should be expanded to all relevant values and maybe also logged to something better than console
        println!("opcode {0:x} | tempBytes {1:x} {2:x} | a 0x{3:x}", self.opcode, self.temp_bytes[0], self.temp_bytes[1], self.a);

        self.cycle += 1;

        if reset {
            self.cycle = 0; 
        }
    }

    fn missing_opcode(&mut self) {
        self.halted = true; 
        println!("Encountered unimplemented Opcode 0x{:x}", self.opcode)
    }

    fn increment_pc(&mut self) {
        self.program_counter += 1;
    }

    fn take_branch(&mut self, val: u8) {
        let mut signed_val: i16 = val as i16;
        if signed_val > 127 {
            signed_val -= 256;
            self.program_counter += signed_val as u16;
            self.cycle += 1;
        }
        else
        {
            self.program_counter += val as u16;
        }
    }

    fn set_flags(&mut self, val: u8) {
        self.flags.flag_zero = val == 0;
        self.flags.flag_negative = val > 127
    }
}
