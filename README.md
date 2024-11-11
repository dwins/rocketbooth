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

To build the Rust sources, use 
```sh
cargo build --release
```
See the Cargo documentation for more information.

## Configuration
Some configuration is required to adapt the application to specific hardware.
A sample configuration is provided in the ``rocketbooth.cfg`` file in this repository and was tested with the hardware listed above.
See the API documentation for the PhotoboothConfig struct for information about the configuration options and how to determine appropriate values for your setup.

Additionally, the image assets used for the photobooth UI are simple PNG files on disk and can be replaced to customize the display.
These are found in a directory named `prompts` in the same directory as the configuration file.

* `prompts/prompts.001.png` is the "title" card displayed while the photobooth is idle, waiting for a user to initiate the photo timer.
* `prompts/prompts.002.png` is an instruction card overlayed on the live preview.  This gives the user some time to make sure the photo is well framed before starting the timer, and also gives the webcam time to auto-adjust any settings like brightness and focus if it has that feature.
* `prompts/prompts.003.png` through `prompts/prompts.006.png` define the numbers used to count down while the photobooth timer is active.
* `prompts/prompts.007.png` is displayed with some animation to delay for the printer but not give the appearance of the app freezing.

### GPIO

The Raspberry Pi hardware includes a General Purpose Input/Output (GPIO) device for integrating with external electronics. 
Rocketbooth supports using the GPIO pins to advance through the states of the photobooth process - eg by hooking up a button directly to the GPIO pins.

This functionality is **disabled** in the default build!
To enable it, switch on the `gpio` feature when building Rocketbooth, eg 
```sh
cargo build --release -F gpio
```
Rocketbooth currently always looks for a rising edge on the GPIO2 pin, but I'm open to pull requests improving the configurability for this!

## Running
This application is designed to run without an X11 graphical environment and so you can configure your raspberry pi to use console/text mode to have a faster startup time and lower RAM usage.
After building with cargo, you should have an executable in `target/release/main` .
Running this with a config file in the working directory will initiate the photobooth experience, which will keep running until you force the application to stop (with ctrl+c or by sending a TERM signal from another terminal.)
Just touch the screen to activate the photobooth prompts!