MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  RAM   : ORIGIN = 0x20000000, LENGTH = 768K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  .vector_table ORIGIN(FLASH) :
  {
    KEEP(*(.vector_table.reset_vector));
    KEEP(*(.vector_table.exceptions));
  } > FLASH

  .text :
  {
    *(.text .text.*);
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > FLASH

  .data : AT(LOADADDR(.text) + SIZEOF(.text))
  {
    . = ALIGN(4);
    _sdata = .;
    *(.data .data.*);
    . = ALIGN(4);
    _edata = .;
  } > RAM
  _sidata = LOADADDR(.data);

  .bss (NOLOAD) :
  {
    . = ALIGN(4);
    _sbss = .;
    *(.bss .bss.*);
    *(COMMON);
    . = ALIGN(4);
    _ebss = .;
  } > RAM

  .uninit (NOLOAD) :
  {
    . = ALIGN(4);
    *(.uninit .uninit.*);
    . = ALIGN(4);
  } > RAM

  /DISCARD/ :
  {
    *(.ARM.exidx .ARM.exidx.*);
    *(.ARM.extab .ARM.extab.*);
    *(.comment);
  }
}
