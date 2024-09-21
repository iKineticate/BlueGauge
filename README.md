# BlueGauge
A lightweight tray tool for easily checking the battery level of your Bluetooth devices.

一款轻便的托盘工具，可轻松查看蓝牙设备的电池电量。

![image](https://raw.githubusercontent.com/iKineticate/BlueGauge/main/screenshots/app.png)


- [x] 左键单击托盘显示通知
- [x] 支持非低功耗蓝牙设备（PnP设备）
- [ ] 左键点击托盘显示通知
- [ ] 菜单：自定义更新时间
- [ ] 菜单：添加开机启动 
- [ ] 菜单：更新按钮
- [ ] 托盘图标替换为指定蓝牙设备的电量（数字或电池图标）
- [ ] 低电量通知（可选通知阈值）
- [ ] 定时通知指定已连接设备的电量
- [ ] 通知设置：可选静音及其它声音
- [ ] 通知设置：可选是否展示进度条


# 问题：
1. 托盘提示的长度受到限制
2. 托盘提示的行数受到限制
3. 使用PnP获取电量时，CPU使用率过高（≈12%）
4. 当更新托盘时，右键菜单会消失
