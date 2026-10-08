local function main()
  local start_time = os.clock()

  local x = 1000000

  while x > 0 do
    x = x - 1
  end

  -- 0.004 seconds Lua 5.4.6
  print(os.clock() - start_time)
end

main()
