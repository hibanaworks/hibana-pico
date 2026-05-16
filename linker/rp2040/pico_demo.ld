MEMORY
{
  FLASH (rx) : ORIGIN = 0x10000000, LENGTH = 2M
  RAM   (rwx): ORIGIN = 0x20000000, LENGTH = 264K
}

__stack_top = ORIGIN(RAM) + LENGTH(RAM);
__stack_limit = __stack_top - 24K;
__stack_reserved_bytes = __stack_top - __stack_limit;
__core1_stack_top = __stack_top - 8K;

ENTRY(Reset);

SECTIONS
{
  .boot2 ORIGIN(FLASH) :
  {
    KEEP(*(.boot2))
  } > FLASH

  .vector_table ORIGIN(FLASH) + 0x100 :
  {
    KEEP(*(.vector_table.reset_vector))
  } > FLASH

  .text :
  {
    *(.text*)
    *(.rodata*)
  } > FLASH

  .data : ALIGN(4)
  {
    __data_start = .;
    *(.data*)
    __data_end = .;
  } > RAM AT > FLASH
  __data_load_start = LOADADDR(.data);

  .bss (NOLOAD) : ALIGN(4)
  {
    __bss_start = .;
    *(.bss*)
    *(COMMON)
    __bss_end = .;
  } > RAM

  . = ALIGN(8);
  __static_ram_end = .;
}

ASSERT(SIZEOF(.boot2) == 256, "rp2040 flash image must start with a 256-byte boot2 stage")
ASSERT(__static_ram_end <= __stack_limit, "hibana-pico static RAM exceeds the reserved dual-core stack budget")
