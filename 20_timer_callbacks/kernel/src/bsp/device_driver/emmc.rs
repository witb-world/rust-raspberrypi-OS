use crate::{
    bsp::device_driver::common::MMIODerefWrapper, driver, exception::asynchronous::IRQNumber,
};

use tock_registers::{
    interfaces::{ReadWriteable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
    LocalRegisterCopy,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

// EMMC command
// derived from Zach's CS140e starter code, as well as
// https://github.com/rockytriton/LLD/blob/main/rpi_bm/part17/include/peripherals/emmc.h
// and
// https://github.com/nihalpasham/rustBoot/blob/main/boards/hal/src/rpi/rpi4/bsp/drivers/emmc.rs

// EMMC commands are a single-word bitfield
// --------------------------------------------------------------------
// PRIVATE INTERNAL SD HOST REGISTER STRUCTURES AS PER BCM2835 MANUAL
// --------------------------------------------------------------------

// EMMC module registers.

register_bitfields! {
    u32,

    /// BLKSIZECNT register - It contains the number and size in bytes for data blocks to be transferred
    BLKSIZECNT [

            /// EMMC module restricts the maximum block size to the size of the internal data
            /// FIFO which is 1k bytes.
            BLKSIZE OFFSET(0) NUMBITS(10) [],
            /// Reserved - Write as 0, read as don't care
            RESERVED OFFSET(10) NUMBITS(6) [],
            /// BLKCNT is used to tell the host how many blocks of data are to be transferred.
            /// Once the data transfer has started and the TM_BLKCNT_EN bit in the CMDTM register is
            /// set, the EMMC module automatically decreases the BNTCNT value as the data blocks
            /// are transferred and stops the transfer once BLKCNT reaches 0.
            BLKCNT OFFSET(16) NUMBITS(16) [],

    ],

    /// CMDTM register - This register is used to issue commands to the card
    CMDTM [

            /// Reserved - Write as 0, read as don't care
            _reserved OFFSET(0) NUMBITS(1) [],
            ///	Enable the block counter for multiple block transfers
            TM_BLKCNT_EN OFFSET(1) NUMBITS(1) [],
            /// Select the command to be send after completion of a data transfer:
            ///  - 0b00: no command
            ///  - 0b01: command CMD12
            ///  - 0b10: command CMD23
            ///  - 0b11: reserved
            TM_AUTO_CMD_EN OFFSET(2) NUMBITS(2) [
                    TM_NO_CMD = 0b00,
                    TM_CMD12 = 0b01,
                    TM_CMD23 = 0b10,
                    _TM_RESERVED = 0b11
            ],
            /// Direction of data transfer (0 = host to card , 1 = card to host )
            TM_DAT_DIR OFFSET(4) NUMBITS(1) [],
            /// Type of data transfer (0 = single block, 1 = muli block)
            TM_MULTI_BLOCK OFFSET(5) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(6) NUMBITS(9) [],
            /// Type of expected response from card
            CMD_RSPNS_TYPE OFFSET(16) NUMBITS(2) [
                    ///  - 0b00: no response
                    CMD_NO_RESP = 0,
                    ///  - 0b01: 136 bits response
                    CMD_136BIT_RESP = 1,
                    ///  - 0b10: 48 bits response
                    CMD_48BIT_RESP = 2,
                    ///  - 0b11: 48 bits response using busy
                    CMD_BUSY48BIT_RESP = 3
            ],
            /// Write as zero read as don't care
            _reserved2 OFFSET(18) NUMBITS(1) [],
            /// Check the responses CRC (0=disabled, 1= enabled)
            CMD_CRCCHK_EN OFFSET(19) NUMBITS(1) [],
            /// Check that response has same index as command (0=disabled, 1=enabled)
            CMD_IXCHK_EN OFFSET(20) NUMBITS(1) [],
            /// Command involves data transfer (0=disabled, 1=enabled)
            CMD_ISDATA OFFSET(21) NUMBITS(1) [],
            /// Type of command to be issued to the card
            ///  - 0b00: normal command
            ///  - 0b01: suspend command
            ///  - 0b10: resume command
            ///  - 0b11: abort command
            CMD_TYPE OFFSET(22) NUMBITS(2) [
                    CMD_TYPE_NORMAL = 0b00,
                    CMD_TYPE_SUSPEND = 0b01,
                    CMD_TYPE_RESUME = 0b10,
                    CMD_TYPE_ABORT = 0b11
             ],
            /// Index of the command to be issued to the card
            CMD_INDEX OFFSET(24) NUMBITS(6) [],
            /// Write as zero read as don't care
            _reserved3 OFFSET(30) NUMBITS(2) [],
    ],

    /// EMMC STATUS register - This register contains information intended for debugging.
    STATUS [

            /// Command line still used by previous command
            CMD_INHIBIT OFFSET(0) NUMBITS(1) [],
            /// Data lines still used by previous data transfer
            DAT_INHIBIT OFFSET(1) NUMBITS(1) [],
            /// At least one data line is active
            DAT_ACTIVE OFFSET(2) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved OFFSET(3) NUMBITS (5) [],
            /// New data can be written to EMMC
            WRITE_TRANSFER OFFSET(8) NUMBITS(1) [],
            /// New data can be read from EMMC
            READ_TRANSFER OFFSET(9) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(10) NUMBITS (10) [],
            /// Value of data lines DAT3 to DAT0
            DAT_LEVEL0 OFFSET(20) NUMBITS(4) [],
            /// Value of command line CMD
            CMD_LEVEL OFFSET(24) NUMBITS(1) [],
            /// Value of data lines DAT7 to DAT4
            DAT_LEVEL1 OFFSET(25) NUMBITS (4) [],
            /// Write as zero read as don't care
            _reserved2 OFFSET(29) NUMBITS (3) [],
     ],

    /// This register is used to configure the EMMC module.
 CONTROL0 [

            /// LED
            LED OFFSET(0) NUMBITS(1) [],
            /// Use 4 data lines (true = enable)
            HCTL_DWIDTH OFFSET(1) NUMBITS(1) [],
            /// Select high speed mode (true = enable)
            HCTL_HS_EN OFFSET(2) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(3) NUMBITS(2) [],
            /// Use 8 data lines (true = enable)
            HCTL_8BIT OFFSET(5) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved2 OFFSET(6) NUMBITS(2) [],
            /// Buspower
            BUSPOWER OFFSET(8) NUMBITS(1) [],
            /// Busvoltage
            BUSVOLTAGE OFFSET(9) NUMBITS(3) [
                    V1_8 = 0b101,
                    V3_0 = 0b110,
                    V3_3 = 0b111,
            ],
            /// Write as zero read as don't care
            _reserved3 OFFSET(12) NUMBITS(4) [],
            /// Stop the current transaction at the next block gap
            GAP_STOP OFFSET(16) NUMBITS(1) [],
            /// Restart a transaction last stopped using the GAP_STOP
            GAP_RESTART OFFSET(17) NUMBITS(1) [],
            /// Use DAT2 read-wait protocol for cards supporting this
            READWAIT_EN OFFSET(18) NUMBITS(1) [],
            /// Enable SDIO interrupt at block gap
            GAP_IEN OFFSET(19) NUMBITS(1) [],
            /// SPI mode enable
            SPI_MODE OFFSET(20) NUMBITS(1) [],
            /// Boot mode access
            BOOT_EN OFFSET(21) NUMBITS(1) [],
            /// Enable alternate boot mode access
            ALT_BOOT_EN OFFSET(22) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved4 OFFSET(23) NUMBITS(9) [],
    ],

 /// This register is used to configure the EMMC module.
 CONTROL1 [

            /// Clock enable for internal EMMC clocks for power saving
            CLK_INTLEN OFFSET(0) NUMBITS(1) [],
            /// SD clock stable  0=No 1=yes   **read only
            CLK_STABLE OFFSET(1) NUMBITS(1) [],
            /// SD clock enable  0=disable 1=enable
            CLK_EN OFFSET(2) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved OFFSET(3) NUMBITS (2) [],
            /// Mode of clock generation (0=Divided, 1=Programmable)
            CLK_GENSEL OFFSET(5) NUMBITS(1) [],
            /// SD clock base divider MSBs (Version3+ only)
            CLK_FREQ_MS2 OFFSET(6) NUMBITS(2) [],
            /// SD clock base divider LSBs
            CLK_FREQ8 OFFSET(8) NUMBITS(8) [],
            /// Data timeout unit exponent
            DATA_TOUNIT OFFSET(16) NUMBITS(4) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(20) NUMBITS (4) [],
            /// Reset the complete host circuit
            SRST_HC OFFSET(24) NUMBITS(1) [],
            /// Reset the command handling circuit
            SRST_CMD OFFSET(25) NUMBITS(1) [],
            /// Reset the data handling circuit
            SRST_DATA OFFSET(26) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved2 OFFSET(27) NUMBITS (5) [],
    ],

    /// This register is used to enable the different interrupts in the INTERRUPT register to
    /// generate an interrupt on the int_to_arm output.
    CONTROL2 [

            /// Auto command not executed due to an error **read only
            ACNOX_ERR OFFSET(0) NUMBITS(1) [],
            /// Timeout occurred during auto command execution **read only
            ACTO_ERR OFFSET(1) NUMBITS(1) [],
            /// Command CRC error occurred during auto command execution **read only
            ACCRC_ERR OFFSET(2) NUMBITS(1) [],
            /// End bit is not 1 during auto command execution **read only
            ACEND_ERR OFFSET(3) NUMBITS(1) [],
            /// Command index error occurred during auto command execution **read only
            ACBAD_ERR OFFSET(4) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved OFFSET(5) NUMBITS(2) [],
            /// Error occurred during auto command CMD12 execution **read only
            NOTC12_ERR OFFSET(7) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(8) NUMBITS(8) [],
            /// Select the speed mode of the SD card (SDR12, SDR25 etc)
            UHSMODE OFFSET(16) NUMBITS(3) [
                SDR12 = 0,
                SDR25 = 1,
                SDR50 = 2,
                SDR104 = 3,
                DDR50 = 4,
             ],
            /// Write as zero read as don't care
            _reserved2 OFFSET(19) NUMBITS(3) [],
            /// Start tuning the SD clock
            TUNEON OFFSET(22) NUMBITS(1) [],
            /// Tuned clock is used for sampling data
            TUNED OFFSET(23) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved3 OFFSET(24) NUMBITS(8) [],
    ],

    /// This register holds the interrupt flags. Each flag can be disabled using the corresponding bit
    /// in the IRPT_MASK register.
    INTERRUPT [

            /// Command has finished
            CMD_DONE OFFSET(0) NUMBITS(1) [],
            /// Data transfer has finished
            DATA_DONE OFFSET(1) NUMBITS(1) [],
            /// Data transfer has stopped at block gap
            BLOCK_GAP OFFSET(2) NUMBITS(1) [],
            /// DMA Interrupt
            DMA_INT OFFSET(3) NUMBITS(1) [],
            /// Data can be written to DATA register
            WRITE_RDY OFFSET(4) NUMBITS(1) [],
            /// DATA register contains data to be read
            READ_RDY OFFSET(5) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(6) NUMBITS(2) [],
            /// Card made interrupt request
            CARD_INT OFFSET(8) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved2 OFFSET(9) NUMBITS(3) [],
            /// Clock retune request was made
            RETUNE OFFSET(12) NUMBITS(1) [],
            /// Boot acknowledge has been received
            BOOTACK OFFSET(13) NUMBITS(1) [],
            /// Boot operation has terminated
            ENDBOOT OFFSET(14) NUMBITS(1) [],
            /// An error has occured
            ERR OFFSET(15) NUMBITS(1) [],
            /// Timeout on command line
            CTO_ERR OFFSET(16) NUMBITS(1) [],
            /// Command CRC error
            CCRC_ERR OFFSET(17) NUMBITS(1) [],
            /// End bit on command line not 1
            CEND_ERR OFFSET(18) NUMBITS(1) [],
            /// Incorrect command index in response
            CBAD_ERR OFFSET(19) NUMBITS(1) [],
            /// Timeout on data line
            DTO_ERR OFFSET(20) NUMBITS(1) [],
            /// Data CRC error
            DCRC_ERR OFFSET(21) NUMBITS(1) [],
            /// End bit on data line not 1
            DEND_ERR OFFSET(22) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved3 OFFSET(23) NUMBITS(1) [],
            /// Auto command error
            ACMD_ERR OFFSET(24) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved4 OFFSET(25) NUMBITS(7) [],
    ],

    /// This register is used to mask the interrupt flags in the INTERRUPT register.
    IRPT_MASK [
            /// Command has finished
            CMD_DONE OFFSET(0) NUMBITS(1) [],
            /// Data transfer has finished
            DATA_DONE OFFSET(1) NUMBITS(1) [],
            /// Data transfer has stopped at block gap
            BLOCK_GAP OFFSET(2) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved OFFSET(3) NUMBITS(1) [],
            /// Data can be written to DATA register
            WRITE_RDY OFFSET(4) NUMBITS(1) [],
            /// DATA register contains data to be read
            READ_RDY OFFSET(5) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(6) NUMBITS(2) [],
            /// Card made interrupt request
            CARD OFFSET(8) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved2 OFFSET(9) NUMBITS(3) [],
            /// Clock retune request was made
            RETUNE OFFSET(12) NUMBITS(1) [],
            /// Boot acknowledge has been received
            BOOTACK OFFSET(13) NUMBITS(1) [],
            /// Boot operation has terminated
            ENDBOOT OFFSET(14) NUMBITS(1) [],
            /// An error has occured
            ERR OFFSET(15) NUMBITS(1) [],
            /// Timeout on command line
            CTO_ERR OFFSET(16) NUMBITS(1) [],
            /// Command CRC error
            CCRC_ERR OFFSET(17) NUMBITS(1) [],
            /// End bit on command line not 1
            CEND_ERR OFFSET(18) NUMBITS(1) [],
            /// Incorrect command index in response
            CBAD_ERR OFFSET(19) NUMBITS(1) [],
            /// Timeout on data line
            DTO_ERR OFFSET(20) NUMBITS(1) [],
            /// Data CRC error
            DCRC_ERR OFFSET(21) NUMBITS(1) [],
            /// End bit on data line not 1
            DEND_ERR OFFSET(22) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved3 OFFSET(23) NUMBITS(1) [],
            /// Auto command error
            ACMD_ERR OFFSET(24) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved4 OFFSET(25) NUMBITS(7) [],
            ],

    IRPT_EN [
            /// Command has finished
            CMD_DONE OFFSET(0) NUMBITS(1) [],
            /// Data transfer has finished
            DATA_DONE OFFSET(1) NUMBITS(1) [],
            /// Data transfer has stopped at block gap
            BLOCK_GAP OFFSET(2) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved OFFSET(3) NUMBITS(1) [],
            /// Data can be written to DATA register
            WRITE_RDY OFFSET(4) NUMBITS(1) [],
            /// DATA register contains data to be read
            READ_RDY OFFSET(5) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved1 OFFSET(6) NUMBITS(2) [],
            /// Card made interrupt request
            CARD OFFSET(8) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved2 OFFSET(9) NUMBITS(3) [],
            /// Clock retune request was made
            RETUNE OFFSET(12) NUMBITS(1) [],
            /// Boot acknowledge has been received
            BOOTACK OFFSET(13) NUMBITS(1) [],
            /// Boot operation has terminated
            ENDBOOT OFFSET(14) NUMBITS(1) [],
            /// An error has occured
            ERR OFFSET(15) NUMBITS(1) [],
            /// Timeout on command line
            CTO_ERR OFFSET(16) NUMBITS(1) [],
            /// Command CRC error
            CCRC_ERR OFFSET(17) NUMBITS(1) [],
            /// End bit on command line not 1
            CEND_ERR OFFSET(18) NUMBITS(1) [],
            /// Incorrect command index in response
            CBAD_ERR OFFSET(19) NUMBITS(1) [],
            /// Timeout on data line
            DTO_ERR OFFSET(20) NUMBITS(1) [],
            /// Data CRC error
            DCRC_ERR OFFSET(21) NUMBITS(1) [],
            /// End bit on data line not 1
            DEND_ERR OFFSET(22) NUMBITS(1) [],
            /// Write as zero read as don't care
            _reserved3 OFFSET(23) NUMBITS(1) [],
            /// Auto command error
            ACMD_ERR OFFSET(24) NUMBITS(1) [],
            /// Write as zero read as don't car
            _reserved4 OFFSET(25) NUMBITS(7) [],
    ],

    /// This register is used to delay the card clock when sampling the returning data and
    /// command response from the card. DELAY determines by how much the sampling clock is delayed per step
    TUNE_STEP [
            /// Select the speed mode of the SD card (SDR12, SDR25 etc)
            DELAY OFFSET(0) NUMBITS(3) [
                    TUNE_DELAY_200ps  = 0,
                    TUNE_DELAY_400ps  = 1,
                    TUNE_DELAY_400psA = 2,
                    TUNE_DELAY_600ps  = 3,
                    TUNE_DELAY_700ps  = 4,
                    TUNE_DELAY_900ps  = 5,
                    // why the duplicate value??
                    TUNE_DELAY_900psA = 6,
                    TUNE_DELAY_1100ps = 7,
            ],
            /// Write as zero read as don't care
            _reserved OFFSET(3) NUMBITS(29) [],
    ],

    /// This register contains the version information and slot interrupt status
    SLOTISR_VER [
            /// Logical OR of interrupt and wakeup signal for each slot
            SLOT_STATUS OFFSET(0) NUMBITS(8) [],
            /// Write as zero read as don't care
            _reserved OFFSET(8) NUMBITS(8) [],
            /// Host Controller specification version
            SDVERSION OFFSET(16) NUMBITS(8) [],
            /// Vendor Version Number
            VENDOR OFFSET(24) NUMBITS(8) [],
    ],
}

register_structs! {
 #[allow(non_snake_case)]
 pub RegisterBlock {
            (0x00 => EMMC_ARG2: ReadWrite<u32>),
            (0x04 => EMMC_BLKSIZECNT: ReadWrite<u32, BLKSIZECNT::Register>),
            (0x08 => EMMC_ARG1: ReadWrite<u32>),
            (0x0c => EMMC_CMDTM: ReadWrite<u32, CMDTM::Register>),
            (0x10 => EMMC_RESP0: ReadWrite<u32>),
            (0x14 => EMMC_RESP1: ReadWrite<u32>),
            (0x18 => EMMC_RESP2: ReadWrite<u32>),
            (0x1c => EMMC_RESP3: ReadWrite<u32>),
            (0x20 => EMMC_DATA:  ReadWrite<u32>),
            (0x24 => EMMC_STATUS: ReadWrite<u32, STATUS::Register>),
            (0x28 => EMMC_CONTROL0: ReadWrite<u32, CONTROL0::Register>),
            (0x2c => EMMC_CONTROL1: ReadWrite<u32, CONTROL1::Register>),
            (0x30 => EMMC_INTERRUPT: ReadWrite<u32, INTERRUPT::Register>),
            (0x34 => EMMC_IRPT_MASK: ReadWrite<u32, IRPT_MASK::Register>),
            (0x38 => EMMC_IRPT_EN: ReadWrite<u32, IRPT_EN::Register>),
            (0x3c => EMMC_CONTROL2: ReadWrite<u32, CONTROL2::Register>),
            (0x40 => _reserved),
            (0x88 => EMMC_TUNE_STEP: ReadWrite<u32, TUNE_STEP::Register>),
            (0x8c => _reserved1),
            (0xfc => EMMC_SLOTISR_VER: ReadWrite<u32, SLOTISR_VER::Register>),
            (0x100 => @END),
    }
}

/// Abstraction for the associated MMIO registers.
type Registers = MMIODerefWrapper<RegisterBlock>;

// --------------------------------------------------------------------------
// SD CARD COMMAND RECORD
// --------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EMMCCommand<'a> {
    cmd_name: &'a str,
    cmd_code: LocalRegisterCopy<u32, CMDTM::Register>,
    use_rca: u16, /* 0-bit of cmd is the rca-bit, subsequent 1-15 bits are reserved i.e. write
                   * as zero read as don't care. */
    delay: u16, // next 16-31 bits contain delay to apply after command.
}

impl<'a> EMMCCommand<'a> {
    const fn new() -> Self {
        EMMCCommand {
            cmd_name: " ",
            cmd_code: LocalRegisterCopy::new(0x0),
            use_rca: 0,
            delay: 0,
        }
    }
}

//--------------------------------------------------------------------------
//                         PUBLIC SD RESULT CODES
//--------------------------------------------------------------------------
#[allow(non_camel_case_types)]
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SdResult {
    EMMC_OK,            // NO error
    EMMC_ERROR,         // General non specific SD error
    EMMC_TIMEOUT,       // SD Timeout error
    EMMC_BUSY,          // SD Card is busy
    EMMC_NO_RESP,       // SD Card did not respond
    EMMC_ERROR_RESET,   // SD Card did not reset
    EMMC_ERROR_CLOCK,   // SD Card clock change failed
    EMMC_ERROR_VOLTAGE, // SD Card does not support requested voltage
    EMMC_ERROR_APP_CMD, // SD Card app command failed
    EMMC_CARD_ABSENT,   // SD Card not present
    EMMC_READ_ERROR,
    EMMC_MOUNT_FAIL,
    EMMC_CARD_STATE(u32),
    NONE,
}

// --------------------------------------------------------------------------
// PUBLIC ENUMERATION OF SD CARD TYPE
// --------------------------------------------------------------------------
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
/// SD card types
pub enum SdCardType {
    EMMC_TYPE_UNKNOWN,
    EMMC_TYPE_MMC,
    EMMC_TYPE_1,
    EMMC_TYPE_2_SC,
    EMMC_TYPE_2_HC,
}

static EMMC_TYPE_NAME: [&str; 5] = ["Unknown", "MMC", "Type 1", "Type 2 SC", "Type 2 HC"];

//--------------------------------------------------------------------------
//                        SD CARD COMMAND DEFINITIONS
//--------------------------------------------------------------------------
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, PartialOrd)]
/// SD card commands
pub enum SdCardCommands {
    GO_IDLE_STATE,
    ALL_SEND_CID,
    SEND_REL_ADDR,
    SET_DSR,
    SWITCH_FUNC,
    CARD_SELECT,
    SEND_IF_COND,
    SEND_CSD,
    SEND_CID,
    VOLTAGE_SWITCH,
    STOP_TRANS,
    SEND_STATUS,
    GO_INACTIVE,
    SET_BLOCKLEN,
    READ_SINGLE,
    READ_MULTI,
    SEND_TUNING,
    SPEED_CLASS,
    SET_BLOCKCNT,
    WRITE_SINGLE,
    WRITE_MULTI,
    PROGRAM_CSD,
    SET_WRITE_PR,
    CLR_WRITE_PR,
    SND_WRITE_PR,
    ERASE_WR_ST,
    ERASE_WR_END,
    ERASE,
    LOCK_UNLOCK,
    APP_CMD,
    APP_CMD_RCA,
    GEN_CMD,
    // Commands hereafter require APP_CMD.
    APP_CMD_START,
    SET_BUS_WIDTH,
    EMMC_STATUS,
    SEND_NUM_WRBL,
    SEND_NUM_ERS,
    APP_SEND_OP_COND,
    SET_CLR_DET,
    SEND_SCR,
}

impl SdCardCommands {
    fn get_cmd(&self) -> EMMCCommand<'static> {
        match self {
            Self::GO_IDLE_STATE => EMMCCommand {
                cmd_name: "GO_IDLE_STATE",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x00) + CMDTM::CMD_RSPNS_TYPE::CMD_NO_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::ALL_SEND_CID => EMMCCommand {
                cmd_name: "ALL_SEND_CID",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x02) + CMDTM::CMD_RSPNS_TYPE::CMD_136BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SEND_REL_ADDR => EMMCCommand {
                cmd_name: "SEND_REL_ADDR",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x03) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SET_DSR => EMMCCommand {
                cmd_name: "SET_DSR",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x04) + CMDTM::CMD_RSPNS_TYPE::CMD_NO_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SWITCH_FUNC => EMMCCommand {
                cmd_name: "SWITCH_FUNC",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x06) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::CARD_SELECT => EMMCCommand {
                cmd_name: "CARD_SELECT",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x07) + CMDTM::CMD_RSPNS_TYPE::CMD_BUSY48BIT_RESP,
                    );
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::SEND_IF_COND => EMMCCommand {
                cmd_name: "SEND_IF_COND",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x08) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 100,
            },
            Self::SEND_CSD => EMMCCommand {
                cmd_name: "SEND_CSD",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x09) + CMDTM::CMD_RSPNS_TYPE::CMD_136BIT_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::SEND_CID => EMMCCommand {
                cmd_name: "SEND_CID",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x0a) + CMDTM::CMD_RSPNS_TYPE::CMD_136BIT_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::VOLTAGE_SWITCH => EMMCCommand {
                cmd_name: "VOLT_SWITCH",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x0b) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::STOP_TRANS => EMMCCommand {
                cmd_name: "STOP_TRANS",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x0c) + CMDTM::CMD_RSPNS_TYPE::CMD_BUSY48BIT_RESP,
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SEND_STATUS => EMMCCommand {
                cmd_name: "SEND_STATUS",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x0d) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::GO_INACTIVE => EMMCCommand {
                cmd_name: "GO_INACTIVE",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x0f) + CMDTM::CMD_RSPNS_TYPE::CMD_NO_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::SET_BLOCKLEN => EMMCCommand {
                cmd_name: "SET_BLOCKLEN",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x10) + CMDTM::CMD_RSPNS_TYPE::CMD_NO_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::READ_SINGLE => EMMCCommand {
                cmd_name: "READ_SINGLE",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x11)
                            + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP
                            + CMDTM::CMD_ISDATA.val(1)
                            + CMDTM::TM_DAT_DIR.val(1),
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::READ_MULTI => EMMCCommand {
                cmd_name: "READ_MULTI",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x12)
                            + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP
                            + CMDTM::CMD_ISDATA.val(1)
                            + CMDTM::TM_DAT_DIR.val(1)
                            + CMDTM::TM_BLKCNT_EN.val(1)
                            + CMDTM::TM_MULTI_BLOCK.val(1),
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SEND_TUNING => EMMCCommand {
                cmd_name: "SEND_TUNING",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x13) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SPEED_CLASS => EMMCCommand {
                cmd_name: "SPEED_CLASS",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x14) + CMDTM::CMD_RSPNS_TYPE::CMD_BUSY48BIT_RESP,
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SET_BLOCKCNT => EMMCCommand {
                cmd_name: "SET_BLOCKCNT",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x17) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::WRITE_SINGLE => EMMCCommand {
                cmd_name: "WRITE_SINGLE",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x18)
                            + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP
                            + CMDTM::CMD_ISDATA.val(1),
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::WRITE_MULTI => EMMCCommand {
                cmd_name: "WRITE_MULTI",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x19)
                            + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP
                            + CMDTM::CMD_ISDATA.val(1)
                            + CMDTM::TM_BLKCNT_EN.val(1)
                            + CMDTM::TM_MULTI_BLOCK.val(1),
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::PROGRAM_CSD => EMMCCommand {
                cmd_name: "PROGRAM_CSD",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x1b) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SET_WRITE_PR => EMMCCommand {
                cmd_name: "SET_WRITE_PR",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x1c) + CMDTM::CMD_RSPNS_TYPE::CMD_BUSY48BIT_RESP,
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::CLR_WRITE_PR => EMMCCommand {
                cmd_name: "CLR_WRITE_PR",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x1d) + CMDTM::CMD_RSPNS_TYPE::CMD_BUSY48BIT_RESP,
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SND_WRITE_PR => EMMCCommand {
                cmd_name: "SND_WRITE_PR",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x1e) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::ERASE_WR_ST => EMMCCommand {
                cmd_name: "ERASE_WR_ST",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x20) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::ERASE_WR_END => EMMCCommand {
                cmd_name: "ERASE_WR_END",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x21) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::ERASE => EMMCCommand {
                cmd_name: "ERASE",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x26) + CMDTM::CMD_RSPNS_TYPE::CMD_BUSY48BIT_RESP,
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::LOCK_UNLOCK => EMMCCommand {
                cmd_name: "LOCK_UNLOCK",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x2a) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::APP_CMD => EMMCCommand {
                cmd_name: "APP_CMD",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x37) + CMDTM::CMD_RSPNS_TYPE::CMD_NO_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 100,
            },
            Self::APP_CMD_RCA => EMMCCommand {
                cmd_name: "APP_CMD_RCA",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x37) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::GEN_CMD => EMMCCommand {
                cmd_name: "GEN_CMD",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x38) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            // Commands hereafter require APP_CMD.
            Self::SET_BUS_WIDTH => EMMCCommand {
                cmd_name: "SET_BUS_WIDTH",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x06) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::EMMC_STATUS => EMMCCommand {
                cmd_name: "EMMC_STATUS",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x0d) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 1,
                delay: 0,
            },
            Self::SEND_NUM_WRBL => EMMCCommand {
                cmd_name: "SEND_NUM_WRBL",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x16) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SEND_NUM_ERS => EMMCCommand {
                cmd_name: "SEND_NUM_ERS",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x17) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::APP_SEND_OP_COND => EMMCCommand {
                cmd_name: "APP_SEND_OP_COND",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x29) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 1000,
            },
            Self::SET_CLR_DET => EMMCCommand {
                cmd_name: "SET_CLR_DET",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(CMDTM::CMD_INDEX.val(0x2a) + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP);
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            Self::SEND_SCR => EMMCCommand {
                cmd_name: "SEND_SCR",
                cmd_code: {
                    let mut cmd = LocalRegisterCopy::new(0u32);
                    cmd.write(
                        CMDTM::CMD_INDEX.val(0x33)
                            + CMDTM::CMD_RSPNS_TYPE::CMD_48BIT_RESP
                            + CMDTM::CMD_ISDATA.val(1)
                            + CMDTM::TM_DAT_DIR.val(1),
                    );
                    cmd
                },
                use_rca: 0,
                delay: 0,
            },
            _ => unimplemented!(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

pub struct EMMC {
    // Coming soon!
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl EMMC {
    pub const COMPATIBLE: &'static str = "EMMC";
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------

impl driver::interface::DeviceDriver for EMMC {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}
