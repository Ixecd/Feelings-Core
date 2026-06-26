# iCE40 + GW1N 双板调通记录

> 日期：2026-06-24
> 性质：硬件 bring-up session log，不 commit，回头 merge 进 dual-fpga-prototype.md

---

## 一、iCE40（ICESugar, HX1K-VQ100）

### 关键结论
- **iCELink CDC 口单向**——只输出调试日志（@init），不把键盘输入透传到 UART TX 脚。短接 TX↔RX 裸线，D4 狂闪但终端不回显，根因在此。
- **拖拽烧录可用**——iCELink MSD 盘符 `/Volumes/iCELink/`，copy .bin 进去即触发 @prog/@start/@cdone:1。
- **FPGA 烧录已验证**——LED 常亮测试（D1=99=ON, D4=95=OFF）确认引脚号正确。
- **UART 引脚**：TX=4, RX=3（VQ100 封装，丝印 UART_TX=P4, UART_RX=P6 但 P6 不是 FPGA 球号）。

### 工具链
```
yosys (synth_ice40) → nextpnr-ice40 (--hx1k --package vq100) → icepack → cp to /Volumes/iCELink/
```
- yosys 0.66, nextpnr-ice40, icestorm 1.1 — 全部 brew
- openFPGALoader（brew）不支持 iCELink VID:PID，需从源码编 +CMSIS-DAP

### 待办
- [ ] CH340 USB-UART 模块到货（预计 6/27）
  - CH340 TX → iCE40 RX(ball 3), RX → iCE40 TX(ball 4), GND → GND
  - loopback 测试：`tio -b 9600 /dev/tty.wchusbserial*`
- [ ] iCE40 FPGA 端 UART 协议栈（460800 baud 帧喷到 GW1N）
- [ ] 传感器 I2C 驱动（MAX30102 + AD5933，等主板）

---

## 二、GW1N（GW1N-LV4LQ144）

### 关键结论
- **板子是 GW1N-4**（4.6K LUT），144-pin LQFP 封装。
- **JTAG 脚序已确认**（10 针排针）：
  ```
  1=TMS  2=NC  3=TDO  4=TDI  5=TCK
  6=GND  7=NC  8=3V3  9=VREF  10=GND
  ```
- **板载 CH340 不支持 openFPGALoader 直连**——VID:PID 不在内置列表。
- **串口无 boot 输出**——板子可能是空片或测试固件不占 UART。

### 开源工具链（macOS 全通 ✅）
```
yosys (synth_gowin) → nextpnr-himbaechel → gowin_pack → .fs bitstream
```
- yosys 0.66 — `synth_gowin` 内置 ✅
- nextpnr-himbaechel — 从 https://github.com/YosysHQ/nextpnr 编，`-DARCH=himbaechel -DHIMBAECHEL_UARCH=gowin` ✅
  - 设备名需带 speed grade：`--device GW1N-LV4LQ144C6/I5`
- apycula 0.32 — `pip3 install --break-system-packages` ✅
- openFPGALoader 自编 — 从 https://github.com/trabucayre/openFPGALoader 编，开 CMSIS-DAP ✅
  - 路径：`/tmp/openfpgaloader/build/openFPGALoader`

### 工具链安装记录
```bash
# apycula（Gowin chip database + gowin_pack）
pip3 install --break-system-packages apycula

# nextpnr-himbaechel
git clone https://github.com/YosysHQ/nextpnr.git /tmp/nextpnr
cmake -B build -DARCH=himbaechel -DHIMBAECHEL_UARCH=gowin -DBUILD_PYTHON=OFF
cmake --build build -j$(sysctl -n hw.ncpu)
cp build/nextpnr-himbaechel /opt/homebrew/bin/
mkdir -p /opt/homebrew/share/himbaechel/gowin
cp build/himbaechel/uarch/gowin/chipdb-*.bin /opt/homebrew/share/himbaechel/gowin/

# openFPGALoader（带 CMSIS-DAP + GWU2X）
git clone https://github.com/trabucayre/openFPGALoader.git /tmp/openfpgaloader
brew install hidapi pkgconf
cmake -B build -DCMAKE_BUILD_TYPE=Release -DENABLE_CMSISDAP=ON -DENABLE_GOWIN_GWU2X=ON
cmake --build build
# binary: /tmp/openfpgaloader/build/openFPGALoader
```

### 合成验证
```bash
yosys -p "synth_gowin -top top -json gw1n.json" top.v
nextpnr-himbaechel --device GW1N-LV4LQ144C6/I5 --json gw1n.json --write pnr.json --vopt cst=pin.cst
python3 -c "import sys; sys.argv=['gowin_pack','-d','GW1N-4','-o','top.fs','pnr.json']; from apycula import gowin_pack; gowin_pack.main()"
```
生成 1.1MB 的 .fs bitstream ✅

### 待办
- [ ] CH340 模块到货
  - 方案 A：CH340 RTS/DTR/CTS → GW1N JTAG TCK/TMS/TDI，bit-bang 烧录
  - 方案 B：iCE40 编程为 UART→JTAG 桥，电脑→CH340→iCE40→JTAG→GW1N
- [ ] GW1N 引脚约束文件（CST）——丝印无 LED 标注，需对着板子实际走线或原理图反推
- [ ] ESIR 帧引擎 + 漏桶 Verilog
- [ ] rPLL 24MHz→7.3728MHz 精准过采样时钟

---

## 三、时间线

| 时间 | 事件 |
|---|---|
| 19:42 | iCE40 UART 短接测试，D4 狂闪，确认 FT2232 物理通路 |
| ~20:00 | 装 yosys/icestorm 工具链 |
| ~20:30 | 第一版 loopback Verilog 合成成功，发现 VQ100 pin 6 不是 IO |
| ~21:00 | 拖拽烧录发现 CDC 口单向——根因确认 |
| ~22:00 | LED 常亮测试确认 FPGA 烧录+引脚正确 |
| ~22:30 | iCELink CDC 单向根因确认；下单 CH340 模块 |
| ~23:00 | GW1N 工具链全通（yosys+nextpnr-himbaechel+gowin_pack） |
| ~23:30 | openFPGALoader 自编 +CMSIS-DAP 完成；GW1N JTAG 脚序确认 |

### 消耗总结
- iCE40 烧了 ~8 版 bitstream 才反推出正确的 LED/UART 引脚
- 最大坑：iCELink CDC 只输出不输入——不是任何人的错，是硬件设计边界
- 开源 Gowin 工具链在 macOS 上完全可跑，但需从源码编 nextpnr-himbaechel
