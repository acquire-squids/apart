# # Attribution:
# This file was ported from `../lang/bench/mandelbrot_bench.txt`, which was ported from https://github.com/xacrimon/tcvm/blob/a821dc0b433de3ce33a106cc1c01c3eeae066300/test-files/mandel_bench2.lua
import math, time

class Pixel:
    def __init__(self, x, y, iteration):
        self.x = x
        self.y = y
        self.iteration = iteration

def mandelbrot_pixel(x0, y0):
    width = 800.0
    height = 800.0
    max_iteration = 100

    x = 0.0
    y = 0.0
    x2 = 0.0
    y2 = 0.0
    w = 0.0
    iteration = 0
    x_min = -2.00
    y_min = -1.125
    x_max = 0.475
    y_max = 1.125
    x0 = x_min + (x0 * (x_max - x_min) / width)
    y0 = y_min + (y0 * (y_max - y_min) / height)

    while x2 + y2 <= 4.0 and iteration < max_iteration:
        x = x2 - y2 + x0
        y = w - x2 - y2 + y0
        x2 = x * x
        y2 = y * y
        w = (x + y) * (x + y)

        iteration += 1

    return Pixel(x, y, iteration)

def mandelbrot():
    width = 800
    height = 800
    max_iteration = 100

    cols = 100
    rows = 40
    step_x = width // cols
    step_y = height // rows
    ramp = " .,:;-~+=*oxOX%#&@"
    levels = len(ramp)
    log_max = math.log(max_iteration + 1)
    total_iterations = 0

    for y in range(height):
        row = ""

        sampled_row = y % step_y == 0

        for x in range(width):
            pixel = mandelbrot_pixel(x, y)

            total_iterations += pixel.iteration

            if sampled_row and x % step_x == 0:
                index = levels if pixel.iteration >= max_iteration else math.floor(math.log(pixel.iteration + 1) / log_max * (levels - 1)) + 1

                value = ramp[(index - 1):index]

                row += value

        if sampled_row:
            print(row)

    return total_iterations

def main():
    start_time = time.perf_counter()

    print(mandelbrot())

    print(time.perf_counter() - start_time)

main()
