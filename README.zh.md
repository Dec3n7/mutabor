<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" height="96" alt="Mutabor 图标">
</p>

<h1 align="center">Mutabor</h1>

<p align="center">
  <a href="README.md">English</a> · <b>中文</b> · <a href="README.ru.md">Русский</a> · <a href="README.ja.md">日本語</a> · <a href="README.es.md">Español</a>
</p>

<p align="center">
  两次按键，完成文件的转换与整理。<br>
  免费、开源，你的文件绝不会被上传到任何地方。
</p>

<p align="center">
  <a href="https://github.com/Dec3n7/mutabor/releases/latest"><b>下载 Windows、macOS 或 Linux 版</b></a>
</p>

---

## 使用方法

1. 在资源管理器或访达中选中一个或多个文件。
2. 按下 **Ctrl + Alt + C**（Mac 上为 **⌘ + ⌥ + C**）。
3. 光标旁会弹出一个小菜单，选择要做的事即可。

结果会以新文件名保存在原文件旁边，原文件保持不变。

## 能做什么

- **把视频压到合适的大小。** 适配 Discord、电子邮件或你设定的任何上限：8、10、25、50 MB 等。
- **转换格式。** 视频、音乐、照片、PDF 和电子书。iPhone 照片（HEIC）转 JPG、语音转 MP3、视频转 GIF 等等。
- **快速修改。** 旋转或翻转照片、删除照片中的位置信息、裁剪视频、去掉声音、加速、截取画面。
- **PDF。** 合并多个文件、按页拆分、把页面转成图片、减小文件体积。
- **一次处理很多文件。** 甚至可以选中整个文件夹：文件会排队处理，每次同时处理几个。
- **直接发到聊天里。** 完成后点“复制”，然后在 Telegram、Discord 或任何地方按 Ctrl + V。
- **截图也可以。** 复制一张截图，按下快捷键，就能把它保存为文件或直接转换。
- **自己的快捷键。** 给任意操作设置专属按键，或把几个操作串成一个（“去掉声音，再压到 8 MB”）。

不支持 Word、Excel 和 PowerPoint 文件。

## 安装

打开[最新版本](https://github.com/Dec3n7/mutabor/releases/latest)，下载适合你电脑的文件：

| 你的电脑 | 要下载的文件 |
| --- | --- |
| Windows 10 或 11 | `Mutabor_…_x64-setup.exe` |
| 搭载 Apple 芯片的 Mac（M1 及更新） | `Mutabor_…_aarch64.dmg` |
| 搭载 Intel 处理器的 Mac | `Mutabor_…_x64.dmg` |
| Linux | `Mutabor_…_amd64.AppImage` |

安装后，Mutabor 会安静地待在系统托盘里（Mac 上在屏幕顶部的菜单栏）。它没有主窗口：选中文件、按下快捷键即可。

### 提示“Windows 已保护你的电脑”或“无法打开”

这对一款新的免费软件来说很正常：我们还没有购买代码签名证书，所以系统不认识发布者。

- **Windows：** 点击 **更多信息**，然后点击 **仍要运行**。
- **Mac：** 打开 **系统设置 → 隐私与安全性**，向下滚动并点击 **仍要打开**。

## 安全吗？

- **文件留在你的电脑上。** 不上传任何东西，没有账号，也没有跟踪。
- **只在你同意时联网。** 部分 PDF 和电子书操作需要一个免费的附加组件，Mutabor 会先显示它的大小，经你同意后才下载。
- **一切公开。** 全部源代码都在这个仓库里，安装包由 GitHub 公开构建，而不是在某个人的电脑上制作。

## 小贴士

- 在某个操作上按 **Shift + Enter** 可以调整设置：质量、尺寸、时间范围。
- **设置** 在托盘图标的菜单里：可修改快捷键、语言、同时处理的文件数等。
- 软件支持 13 种语言，默认跟随系统语言。
- **在 Mac 上**，复制截图和 PDF 操作暂不可用。**在 Linux 上**，菜单暂时无法识别你选中的文件，因此目前更适合爱好者尝试。

## 参与贡献

发现了问题或有新想法？欢迎[提交 Issue](https://github.com/Dec3n7/mutabor/issues)。也非常欢迎改进翻译。开发者可以在 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)（英文）中找到构建说明和技术细节。

Mutabor 是基于 [GPL-3.0](LICENSE) 许可证的自由软件。

<sub>名字来自威廉·豪夫的童话《鹳鸟国王》：“Mutabor!”是能让人变形的咒语，拉丁语意为“我将被改变”。</sub>
