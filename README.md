# Rocketbooth

> Run a photobooth from your Raspberry Pi.

Hook up a USB printer and webcam to your Raspberry Pi with touchscreen and Rocketbooth turns it into a photobooth.
Just touch the screen to take a few impromptu photos and immediately print them.

## Hardware
I tested this with:
* [Raspberry Pi Model 3B](https://www.raspberrypi.org/products/raspberry-pi-3-model-b/)
* [Raspberry Pi Touch Display](https://www.raspberrypi.org/products/raspberry-pi-touch-display/)
* [Logitech C920 Pro HD Webcam](https://www.logitech.com/en-us/product/hd-pro-webcam-c920)
* [Canon Selphy CP1300 Photo Printer](https://shop.usa.canon.com/shop/en/catalog/selphy-cp1300-black-wireless-compact-photo-printer)

Other devices should be supported by changing the configuration.

## Building

This project is built with [Cargo](http://cargo.rs), but it depends on some packages from the Raspbian archive which are not included in the default Raspbian install.
The Raspbian package names for these dependencies are listed in the `raspi-packages` file, and can be installed through the apt package manager:
```sh
sudo apt install $(cat raspi-packages)
```

To install Rust and Cargo, use the setup scripts from https://rustup.rs/ .

To build the Rust sources, use `cargo build --release`.
See the Cargo documentation for more information.

## Configuration
Some configuration is required to adapt the application to specific hardware.
A sample configuration is provided in the ``Rocketbooth.toml`` file in this repository and was tested with the hardware listed above.

### Video Connection 
Rocketbooth uses libraries provided by ffmpeg to connect to the webcam, and the ffmpeg command line tool is helpful to test connection settings.  For example
`ffmpeg -f v4l2 -framerate 10 -i /dev/video0 -pix_fmt bgra -f fbdev /dev/fb0` shows the video stream from my webcam. In the configuration file:
- `-i /dev/video0` becomes `format = "v4l2"` in the `[video]` section.
- `-f v4l2` becomes `format = "v4l2"` in the `[video]` section.
- `-framerate 10` becomes `framerate = "10"` in the `[video.options]` section.

Arguments after `-i /dev/video0` affect display and don't have equivalents in Rocketbooth.toml.

See also [ffmpeg documentation on webcams](https://trac.ffmpeg.org/wiki/Capture/Webcam)

### Image Assets

Additionally, the image assets used for the photobooth UI are simple PNG files on disk and can be replaced to customize the display.
These are found in a directory named `prompts` in the same directory as the configuration file.

* `prompts/prompts.001.png` is the "title" card displayed while the photobooth is idle, waiting for a user to initiate the photo timer.
* `prompts/prompts.002.png` is an instruction card overlayed on the live preview.  This gives the user some time to make sure the photo is well framed before starting the timer, and also gives the webcam time to auto-adjust any settings like brightness and focus if it has that feature.
* `prompts/prompts.003.png` through `prompts/prompts.006.png` define the numbers used to count down while the photobooth timer is active.
* `prompts/prompts.007.png` is displayed with some animation to delay for the printer but not give the appearance of the app freezing.

## Running
This application is designed to run without an X11 graphical environment and so you can configure your raspberry pi to use console/text mode to have a faster startup time and lower RAM usage.
After building with cargo, you should have an executable in `target/release/main` .
Running this with a config file in the working directory will initiate the photobooth experience, which will keep running until you force the application to stop (with ctrl+c or by sending a TERM signal from another terminal.)
Just touch the screen to activate the photobooth prompts!