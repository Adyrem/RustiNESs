//Next steps:
// Implement arithmetic functions starting with 0x0A ASL
use std::collections::VecDeque;

pub struct Flags {
    flag_carry: bool,
    flag_zero: bool,
    flag_interrupt_disable: bool,
    flag_decimal: bool,
    flag_overflow: bool,
    flag_negative: bool,
}

pub struct Emulator {
    queue: VecDeque<Box<dyn FnMut(&mut Self)>>,
    ram: [u8; 0x800],
    rom_header: [u8; 16],
    rom: [u8; 0x8000],
    program_counter: u16,
    stack_pointer: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub halted: bool,
    opcode: u8,
    temp_bytes: [u8; 4], // Can be used to store values inbetween cpu cycles, not to be confused with registers
    flags: Flags
}

impl Emulator {
    pub fn new(rom_all: [u8; 0x8010]) -> Self {
        let mut ret = Self { 
            queue: VecDeque::new(),
            ram: [0; 0x800],
            rom_header: rom_all[0..0x10].try_into().expect("Error reading ROM header"),
            rom: rom_all[0x10..0x8010].try_into().expect("Error reading ROM contents"),
            program_counter: 0,
            stack_pointer: 0xFD, //Will apperantly be explained later why its this value
            a: 0,
            x: 0,
            y: 0,
            halted: false, 
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

        //Some operations take more than 1 cycle per operation, e.g. pull which takes 3 cycles for a single method call + 1 for the actual PLA instruction
        // Each operation pushes its cycles into the queue, which are then handled one cycle at a time
        if let Some(mut task) = self.queue.pop_front() {
            task(self);
        } else {
            //If no operations are scheduled, the next opcode should be read
            self.opcode = self.read(self.program_counter);
            self.increment_pc();
            self.queue_Opcode();
        }

        //TODO this should be expanded to all relevant values and maybe also logged to something better than console
        println!("opcode {0:x} | tempBytes {1:x} {2:x} | pc 0x{3:x} | a 0x{4:x}",
         self.opcode, self.temp_bytes[0], self.temp_bytes[1], self.program_counter, self.a);
    }

    //No completely sure how this actually works, thanks chatgpt
    fn add_task<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self) + 'static,
    {
        self.queue.push_back(Box::new(f));
    }

    fn add_task_front<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self) + 'static,
    {
        self.queue.push_front(Box::new(f));
    }

    //This can be called when a function needs to take up multiple cycles but its not clear where the cycles come from
    // Should only be used if no other solution is clear
    fn add_NOP(&mut self)
    {
        self.add_task_front(|s| {});
    }

    fn queue_Opcode(&mut self) {
        match self.opcode {
                0x02 => //HLT
                    self.add_task(|s| s.halted = true),
                0x08 => //PHP
                    self.add_task(|s| s.push_flags()),
                0x28 => //PLP
                    self.add_task(|s| s.pull_flags()),
                0x20 => //JSR
                { 
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| s.temp_bytes[1] = s.read(s.program_counter));
                    self.add_task(|s| s.push((s.program_counter/256) as u8));
                    self.add_task(|s| s.push(s.program_counter as u8));
                    self.add_task(|s| s.program_counter = s.temp_bytes_to_little_endian());
                },
                0x60 => //RTS
                {
                    self.add_task(|s| s.temp_bytes[0] = s.pull());
                    self.add_task(|s| s.temp_bytes[1] = s.pull());
                    self.add_task(|s| { s.program_counter = s.temp_bytes_to_little_endian(); s.increment_pc() } );
                },
                0x4C => //JMP 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { s.temp_bytes[1] = s.read(s.program_counter); s.program_counter = s.temp_bytes_to_little_endian() } );
                },
                0x10 => //BPL
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if !s.flags.flag_negative { s.take_branch(s.temp_bytes[0]) } } );
                },
                0x30 => //BMI
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if s.flags.flag_negative { s.take_branch(s.temp_bytes[0]) } } );
                },
                0x50 => //BVC
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if !s.flags.flag_overflow { s.take_branch(s.temp_bytes[0]) } } );
                },
                0x70 => //BVS 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if s.flags.flag_overflow { s.take_branch(s.temp_bytes[0]) } } );
                },
                0x90 => //BCC 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if !s.flags.flag_carry { s.take_branch(s.temp_bytes[0]) } } );
                },
                0xB0 => //BCS 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if s.flags.flag_carry { s.take_branch(s.temp_bytes[0]) } } );
                },
                0xD0 => //BNE 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if !s.flags.flag_zero { s.take_branch(s.temp_bytes[0]) } } );
                },
                0xF0 => //BEQ 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { if s.flags.flag_zero { s.take_branch(s.temp_bytes[0]) } } );
                },
                0x84 => //STY Zero page 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { s.write(s.temp_bytes[0] as u16, s.y); } );
                },
                0x85 => //STA Zero page 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { s.write(s.temp_bytes[0] as u16, s.a); } );
                },
                0x86 => //STX Zero page 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { s.write(s.temp_bytes[0] as u16, s.x); } );
                },
                0x8C => //STY Absolute 
                {
                    self.read_little_endian_to_temp_bytes();
                    self.add_task(|s| { s.write(s.temp_bytes_to_little_endian(), s.y); } );
                },
                0x8D => //STA Absolute 
                {
                    self.read_little_endian_to_temp_bytes();
                    self.add_task(|s| { s.write(s.temp_bytes_to_little_endian(), s.a); } );
                },
                0x8E => //STX Absolute 
                {
                    self.read_little_endian_to_temp_bytes();
                    self.add_task(|s| { s.write(s.temp_bytes_to_little_endian(), s.x); } );
                },
                0xA5 => //LDA Zero page 
                {
                    self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
                    self.add_task(|s| { s.a = s.read(s.temp_bytes[0] as u16); s.set_flags(s.a) } );
                },
                0xAD => //LDA Absolute 
                {
                    self.read_little_endian_to_temp_bytes();
                    self.add_task(|s| { s.a = s.read(s.temp_bytes_to_little_endian()); s.set_flags(s.a) } );
                },
                0x18 => //CLC
                    self.add_task(|s| s.flags.flag_carry = false), 
                0x38 => //SEC
                    self.add_task(|s| s.flags.flag_carry = true ), 
                0x58 => //CLI
                    self.add_task(|s| s.flags.flag_interrupt_disable = false ), 
                0x78 => //SEI
                    self.add_task(|s| s.flags.flag_interrupt_disable = true ), 
                0xB8 => //CLV
                    self.add_task(|s| s.flags.flag_overflow = true ), 
                0xD8 => //CLD
                    self.add_task(|s| s.flags.flag_decimal = false ), 
                0xF8 => //SED
                    self.add_task(|s| s.flags.flag_decimal = true ), 
                0xEA => //NOP
                    self.add_task(|s| {}), 
                0x48 => //PHA
                   self.add_task(|s| s.push(s.a)), 
                0x9A => //TXS
                    self.add_task(|s| s.stack_pointer = s.x ), 
                0x68 => //PLA
                    self.add_task(|s| s.a = s.pull()),
                0x8A => //TXA
                    self.add_task(|s| { s.a = s.x; s.set_flags(s.a) }), 
                0x98 => //TYA
                  self.add_task(|s| { s.a = s.y; s.set_flags(s.a) } ), 
                0xA9 => //LDA Immediate
                    self.add_task(|s| { s.a = s.read(s.program_counter); s.set_flags(s.a); s.increment_pc() } ),
                0xA0 => //LDY Immediate
                    self.add_task(|s| { s.y = s.read(s.program_counter); s.increment_pc() } ),
                0xA8 => //TAY
                    self.add_task(|s| { s.y = s.a; s.set_flags(s.y) } ), 
                0x88 => //DEY
                    self.add_task(|s| { s.y -= 1; s.set_flags(s.x) } ), 
                0xC8 => //INY
                    self.add_task(|s| { s.y += 1; s.set_flags(s.y) } ), 
                0xAA => //TAX
                    self.add_task(|s| { s.x = s.a; s.set_flags(s.x) } ), 
                0xA2 => //LDX Immediate
                    self.add_task(|s| { s.x = s.read(s.program_counter); s.increment_pc() } ),
                0xBA => //TSX
                    self.add_task(|s| { s.x = s.stack_pointer; s.set_flags(s.x) } ), 
                0xCA => //DEX
                    self.add_task(|s| { s.x -= 1; s.set_flags(s.x) } ), 
                0xE8 => //INX
                    self.add_task(|s| { s.x += 1; s.set_flags(s.x) } ), 
                _ => //Not implemented 
                    self.add_task(|s| s.missing_opcode())
            };

    }
    
    fn read_little_endian_to_temp_bytes(&mut self) {
        self.add_task(|s| { s.temp_bytes[0] = s.read(s.program_counter); s.increment_pc() } );
        self.add_task(|s| { s.temp_bytes[1] = s.read(s.program_counter); s.increment_pc() } );
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
            self.add_NOP();
        }
        else
        {
            self.program_counter += val as u16;
        }
    }

    fn push(&mut self, val: u8) {
        self.write(0x100 + self.stack_pointer as u16, val);
        self.stack_pointer -= 1;
        self.add_NOP();
    }

    fn pull(&mut self) -> u8 {
        self.stack_pointer += 1;
        let ret_val = self.read(0x100 + self.stack_pointer as u16);
        self.add_NOP();
        self.add_NOP();
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
