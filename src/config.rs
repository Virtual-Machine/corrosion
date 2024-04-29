// mod config.rs
// A module centralizing all project configuration

// Main Configuration
pub const VERSION: &str = "v0.2.0";
pub const PLATFORM: &str = "RISCV-64 QEMU Virt";
pub const PAGE_SIZE: usize = 0x1000;
pub const BANNER: &str = "
                              _             
                             (_)            
  ___ ___  _ __ _ __ \x1b[38;5;202m___  ___\x1b[39m _  ___  _ __  
 / __/ _ \\| '__| '__\x1b[38;5;202m/ _ \\/ __|\x1b[39m |/ _ \\| '_ \\ 
| (_| (_) | |  | | \x1b[38;5;202m| (_) \\__ \\\x1b[39m | (_) | | | |
 \\___\\___/|_|  |_|  \x1b[38;5;202m\\___/|___/\x1b[39m_|\\___/|_| |_|
============================================\n";


// Colour Print Labels
pub const MAIN: &str = "[\x1b[38;5;214mMAIN\x1b[39m]";
pub const STEP: &str = "[\x1b[38;5;130mSTEP\x1b[39m]";
pub const INFO: &str = "[\x1b[38;5;167mINFO\x1b[39m]";
pub const TEST: &str = "[\x1b[38;5;202mTEST\x1b[39m]";
pub const DEBUG: &str = "[\x1b[38;5;97mDEBUG\x1b[39m]";
pub const TEST_PASSED: &str = "  ... [\x1b[38;5;41mPASSED\x1b[39m]";
pub const TRAP_COLOUR: &str = "\x1b[38;5;222m";
pub const RESET_COLOUR: &str = "\x1b[39m";
