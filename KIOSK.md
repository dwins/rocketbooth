# Rocketbooth Kiosk Mode

To simplify operating Rocketbooth at events I've done some work to enable what I call "Kiosk" mode.
In a kiosk deployment, Rocketbooth:
- Automatically starts at boot
- Reads configuration from a USB device
- Stores captured photos on the same USB device.

## Setting up kiosk mode

1. Build rocketbooth
2. Ensure rocketbooth binary is in `/home/pi/rocketbooth/rocketbooth`
3. Install the rocketbooth service definition, `rocketbooth.service` in the root of this repository.
4. Ensure usb devices will be auto-mounted.
   On Linux this is typically handled by desktop environments, but there is a package in the raspbian repository that provides this even in a headless system. `apt install usbmount`.

## Preparing a removable drive

Rocketbooth will look for a config file in (usb device)/rocketbooth/Rocketbooth.toml .  The photos captured by the photobooth will be captured to the same directory.

The photobooth prompts should also be in (usb device)/rocketbooth/prompts/ with names as follows:
- prompts.001.png - a welcome screen that shows when the device is woken from  a blanked screen.
- prompts.002.png - instructions that will be shown over a camera preview. Should have some transparency to allow the preview to be used.
- prompts.003-006.png - countdown images for when the photobooth is actually taking pictures.
- prompts.007 - Information to show while waiting on the printer.

