# xmb_lib
A Rust library for reading and writing XMB files. These files are used by Smash Ultimate, Smash 4, and potentially other games. Currently only little endian is supported, so most Smash 4 XMB files will not work.

## xmb
A command line program for converting XMB files to and from XML. The XML output uses the same XML format as the Python script for SSBU-Tools. For a list of files that don't rebuild correctly, see https://github.com/ultimate-research/xmb_lib/issues/8.

The example below shows a `model.xmb` file after converting to XML.  
```xml
<?xml version="1.0" encoding="UTF-8"?>
<model type="effect_main">
    <shadow caster="0"/>
    <lightset number="0"/>
    <object type="2"/>
    <this_light action="0" color="0.000000, 0.000000, 0.000000" local_offset="0, 0, 0" offset="0.0" radius="20.0"/>
    <draw>
        <draw buffer="0" type="main"/>
        <draw action="1" type="normalmap"/>
    </draw>
    <posteffect>
        <reflection search="100.0"/>
    </posteffect>
    <stencil_type number="1"/>
</model>
```

### Usage
A prebuilt binary for Windows is available in [releases](https://github.com/ultimate-research/xmb_lib/releases).  
Drag an XMB or XML file onto the executable or specify the input and output files from the command line. The output is optional and defaults to converting XMB to XML and XML to XMB.   
`xmb.exe <input> [output]`  
`xmb.exe model.xmb model.xml`  
`xmb.exe model.xmb`  
`xmb.exe model.xml model.xmb`  
`xmb.exe model.xml`  

# Credits
[SSBU-Tools](https://github.com/Sammi-Husky/SSBU-TOOLS) | [License](https://github.com/Sammi-Husky/SSBU-TOOLS/blob/master/LICENSE)- Original Python implementation for converting XMB to and from XML
