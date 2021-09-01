# xmb_lib
A Rust library for reading and writing XMB files. These files are used by Smash Ultimate, Smash 4, and potentially other games. Currently only little endian is supported, so most Smash 4 XMB files will not work.

## xmb
A command line program for converting XMB files to and from XML. The XML output uses the same XML format as the Python script for SSBU-Tools. For a list of files that don't rebuild correctly, see https://github.com/ultimate-research/xmb_lib/issues/8.

### Usage
A prebuilt binary for Windows is available in [releases](https://github.com/ultimate-research/xmb_lib/releases).  
Drag and XMB or XML file onto the executable or specify the input and output files from the command line. The output is optional and defaults to converting XMB to XML and XML to XMB.   
`xmb.exe <input> [output]`  
`xmb.exe model.xmb model.xml`  
`xmb.exe model.xmb`  
`xmb.exe model.xml model.xmb`  
`xmb.exe model.xml`  

# Credits
[SSBU-Tools](https://github.com/Sammi-Husky/SSBU-TOOLS) | [License](https://github.com/Sammi-Husky/SSBU-TOOLS/blob/master/LICENSE)- Original Python implementation for converting XMB to and from XML
