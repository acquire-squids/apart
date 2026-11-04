-- # Attribution:
-- This file was ported from `../lang/bench/mandelbrot_bench.txt`, which was ported from https://github.com/xacrimon/tcvm/blob/a821dc0b433de3ce33a106cc1c01c3eeae066300/test-files/mandel_bench2.lua
local function mandelbrot_pixel(x0, y0)
    local width = 800.0
    local height = 800.0
    local max_iteration = 100

    local x = 0.0
    local y = 0.0
    local x2 = 0.0
    local y2 = 0.0
    local w = 0.0
    local iteration = 0

    local x_min = -2.00
    local y_min = -1.125
    local x_max = 0.475
    local y_max = 1.125
    local x0 = x_min + (x0 * (x_max - x_min) / width)
    local y0 = y_min + (y0 * (y_max - y_min) / height)

    while x2 + y2 <= 4.0 and iteration < max_iteration do
        x = x2 - y2 + x0
        y = w - x2 - y2 + y0
        x2 = x * x
        y2 = y * y
        w = (x + y) * (x + y)

        iteration = iteration + 1
    end

    return {
        x = x,
        y = y,
        iteration = iteration
    }
end

local function mandelbrot()
    local width = 800
    local height = 800
    local max_iteration = 100

    local cols = 100
    local rows = 40
    local step_x = math.floor(width / cols)
    local step_y = math.floor(height / rows)
    local ramp = " .,:;-~+=*oxOX%#&@"
    local levels = #ramp
    local log_max = math.log(max_iteration + 1.0)
    local total_iterations = 0

    for y = 0, height - 1 do
        local row = ""

        local sampled_row = y % step_y == 0

        for x = 0, width - 1 do
            local pixel = mandelbrot_pixel(x, y)

            total_iterations = total_iterations + pixel.iteration

            if sampled_row and x % step_x == 0 then
                local index = levels

                if pixel.iteration < max_iteration then
                    index = math.floor(math.log(pixel.iteration + 1) / log_max * (levels - 1)) + 1
                end

                local value = ramp:sub(index, index)

                row = row .. value
            end
        end

        if sampled_row then
            print(row)
        end
    end

    return total_iterations
end

local function main()
    local start_time = os.clock()

    local n = 35

    print(mandelbrot())

    print(os.clock() - start_time)
end

main()
