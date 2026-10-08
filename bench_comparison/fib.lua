local function fib(n)
  if n <= 1 then
    return n
  end

  return fib(n - 2) + fib(n - 1)
end

local function main()
  local start_time = os.clock()

  local n = 35

  print(fib(n))

  -- 0.359 seconds Lua 5.4.6
  print(os.clock() - start_time)
end

main()
