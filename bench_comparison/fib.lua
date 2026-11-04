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

  print(os.clock() - start_time)
end

main()
