MEMORY
{
  FLASH (rx) : ORIGIN = 0x10000000, LENGTH = 4M
  RAM   (rwx): ORIGIN = 0x20000000, LENGTH = 520K
}

__stack_top = ORIGIN(RAM) + LENGTH(RAM);
__stack_limit = __stack_top - 48K;
__stack_reserved_bytes = __stack_top - __stack_limit;
__core1_stack_top = __stack_top - 24K;

ENTRY(Reset);

SECTIONS
{
  .vector_table ORIGIN(FLASH) :
  {
    KEEP(*(.vector_table.reset_vector))
  } > FLASH

  .start_block : ALIGN(4)
  {
    KEEP(*(.start_block))
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

ASSERT(__static_ram_end <= __stack_limit, "hibana-pico2w static RAM exceeds the reserved 48 KiB dual-core stack budget")
