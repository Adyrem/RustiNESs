
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
    stack_pointer: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub halted: bool,
    running_cycle: u8,
    no_op_cycle: u8,
    total_cycles: u8,
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
            stack_pointer: 0xFD, //Will apperantly be explained later why its this value
            a: 0,
            x: 0,
            y: 0,
            halted: false, 
            running_cycle: 0, 
            no_op_cycle: 0, 
            total_cycles: 0, 
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

        panic!("Error reading from RAM or ROM");
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

        //Some operations take more than 1 cycle per operation, e.g. pull which takes 3 cycles for a single method call + 1 for the actual PLA instruction
        // This will need more reworking in the future for better accuracy
        if self.no_op_cycle > 0 {
            self.no_op_cycle -= 1;
            self.total_cycles += 1;
            return;
        }

        if self.running_cycle == 0 {
            self.opcode = self.read(self.program_counter);
            reset = false;
            self.increment_pc();
        }
        else if self.running_cycle == 1 {
            match self.opcode {
                0x02 => self.halted = true,
                0x08 => self.push_flags(),
                0x28 => self.pull_flags(),
                0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0 //BPL, BMI, BVC, BVS, BCC, BCS, BNE, BEQ
                 | 0x20 | 0x4C //JSR, JMP
                 | 0x84..0x86 //STY, STA, STX Zero page
                 | 0x8C..0x8E //STY, STA, STX Absolute
                 | 0xA5 //LDA Zero page
                 | 0xAD //LDA Absolute
                 => { self.temp_bytes[0] = self.read(self.program_counter); reset = false; self.increment_pc();}, 
                0x18 => { self.flags.flag_carry = false } //CLC
                0x38 => { self.flags.flag_carry = true } //SEC
                0x58 => { self.flags.flag_interrupt_disable = false } //CLI
                0x78 => { self.flags.flag_interrupt_disable = true } //SEI
                0xB8 => { self.flags.flag_overflow = true } //CLV
                0xD8 => { self.flags.flag_decimal = false } //CLD
                0xF8 => { self.flags.flag_decimal = true } //SED
                0xEA => { } //NOP
                0x48 => { self.push(self.a); } //PHA
                0x60 => { self.temp_bytes[0] = self.pull(); reset = false;} //RTS
                0x68 => { self.a = self.pull();} //PLA
                0x88 => { self.y -= 1; self.set_flags(self.x);}, //DEY
                0x8A => { self.a = self.x; self.set_flags(self.a);}, //TXA
                0x98 => { self.a = self.y; self.set_flags(self.a);}, //TYA
                0x9A => { self.stack_pointer = self.x }, //TXS
                0xA0 => { self.y = self.read(self.program_counter); self.increment_pc();}, //LDY Immediate
                0xA2 => { self.x = self.read(self.program_counter); self.increment_pc();}, //LDX Immediate
                0xA9 => { self.a = self.read(self.program_counter); self.set_flags(self.a); self.increment_pc();}, //LDA Immediate
                0xA8 => { self.y = self.a; self.set_flags(self.y);}, //TAY
                0xAA => { self.x = self.a; self.set_flags(self.x);}, //TAX
                0xBA => { self.x = self.stack_pointer; self.set_flags(self.x);}, //TSX
                0xC8 => { self.y += 1; self.set_flags(self.y);}, //INY
                0xCA => { self.x -= 1; self.set_flags(self.x);}, //DEX
                0xE8 => { self.x += 1; self.set_flags(self.x);}, //INX
                _ => {self.missing_opcode();}
            };

        }
        else if self.running_cycle == 2 {
            match self.opcode {
                0x10 => { if !self.flags.flag_negative { self.take_branch(self.temp_bytes[0]) } ; }, //BPL
                0x20 => { self.temp_bytes[1] = self.read(self.program_counter); reset = false; }, //JSR 
                0x30 => { if self.flags.flag_negative { self.take_branch(self.temp_bytes[0]) } ; }, //BMI
                0x4C => { self.temp_bytes[1] = self.read(self.program_counter); self.program_counter = self.temp_bytes_to_little_endian() }, //JSR 
                0x60 => { self.temp_bytes[1] = self.pull(); reset = false;} //RTS
                0x84 => { self.write(self.temp_bytes[0] as u16, self.y); }, //STY Zero page
                0x85 => { self.write(self.temp_bytes[0] as u16, self.a); }, //STA Zero page
                0x86 => { self.write(self.temp_bytes[0] as u16, self.x); }, //STX Zero page
                0x8C..0x8E //STY, STA, STX Absolute
                | 0xAD //LDA Absolute
                 => { self.temp_bytes[1] = self.read(self.program_counter); reset = false; self.increment_pc(); }, 
                0xA5 => { self.a = self.read(self.temp_bytes[0] as u16); self.set_flags(self.a) }, //LDA Zero page
                0xD0 => { if !self.flags.flag_zero { self.take_branch(self.temp_bytes[0]) } ; }, //BNE
                0xF0 => { if self.flags.flag_zero { self.take_branch(self.temp_bytes[0]) } ; }, //BEQ
                _ => {self.missing_opcode();}
            };
        }
        else if self.running_cycle == 3 {
            match self.opcode {
                0x20 => { self.push((self.program_counter/256) as u8); reset = false; } //JSR
                0x60 => { self.program_counter = self.temp_bytes_to_little_endian(); self.increment_pc();} //RTS
                0x8C => { self.write(self.temp_bytes_to_little_endian(), self.y); }, //STY Absolute 
                0x8D => { self.write(self.temp_bytes_to_little_endian(), self.a); }, //STA Absolute
                0x8E => { self.write(self.temp_bytes_to_little_endian(), self.x); }, //STX Absolute
                0xAD => { self.a = self.read(self.temp_bytes_to_little_endian()); self.set_flags(self.a)}, //LDA Absolute
                _ => {self.missing_opcode();}
            };
        }
        else if self.running_cycle == 4 {
            match self.opcode {
                0x20 => { self.push(self.program_counter as u8); reset = false; } //JSR
                _ => {self.missing_opcode();}
            };
        }
        else if self.running_cycle == 5 {
            match self.opcode {
                0x20 => { self.program_counter = self.temp_bytes_to_little_endian()} //JSR
                _ => {self.missing_opcode();}
            };
        }

        //TODO this should be expanded to all relevant values and maybe also logged to something better than console
        println!("opcode {0:x} | tempBytes {1:x} {2:x} | pc 0x{3:x} | a 0x{4:x}",
         self.opcode, self.temp_bytes[0], self.temp_bytes[1], self.program_counter, self.a);

        self.running_cycle += 1;
        self.total_cycles += 1;

        if reset {
            self.running_cycle = 0; 
            self.no_op_cycle = 0;
            self.total_cycles = 0;
        }
    }

    fn temp_bytes_to_little_endian(&self) -> u16 {
        return self.temp_bytes[1] as u16 * 0x100 + self.temp_bytes[0] as u16;
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
            self.no_op_cycle += 1;
        }
        else
        {
            self.program_counter += val as u16;
        }
    }

    fn push(&mut self, val: u8) {
        self.write(0x100 + self.stack_pointer as u16, val);
        self.stack_pointer -= 1;
        self.no_op_cycle += 1;
    }

    fn pull(&mut self) -> u8 {
        self.stack_pointer += 1;
        let ret_val = self.read(0x100 + self.stack_pointer as u16);
        self.no_op_cycle += 2;
        return ret_val;
    }

    fn push_flags(&mut self) {
        let mut flag_bytes: u8 = 0;
        flag_bytes += if self.flags.flag_carry { 1 } else { 0 };
        flag_bytes += if self.flags.flag_zero { 2 } else { 0 };
        flag_bytes += if self.flags.flag_interrupt_disable { 4 } else { 0 };
        flag_bytes += if self.flags.flag_decimal { 8 } else { 0 };
        flag_bytes += 16;
        flag_bytes += 32;
        flag_bytes += if self.flags.flag_overflow { 64 } else { 0 };
        flag_bytes += if self.flags.flag_negative { 128 } else { 0 };
        self.push(flag_bytes);
    }

    fn pull_flags(&mut self){
        let flag_bytes: u8 = self.pull();
        self.flags.flag_carry = flag_bytes & 1 != 0;
        self.flags.flag_zero = flag_bytes & 2 != 0;
        self.flags.flag_interrupt_disable = flag_bytes & 4 != 0;
        self.flags.flag_decimal = flag_bytes & 8 != 0;
        //16
        //32
        self.flags.flag_overflow = flag_bytes & 64 != 0;
        self.flags.flag_negative = flag_bytes & 128 != 0;
    }

    fn set_flags(&mut self, val: u8) {
        self.flags.flag_zero = val == 0;
        self.flags.flag_negative = val > 127
    }
}
