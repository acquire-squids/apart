local function is_prime(n)
  if n < 2 or (n ~= 2 and n % 2 == 0) then
    return false
  end

  for i = 3, n - 1, 2 do
    if n % i == 0 then
      return false
    end
  end

  return true
end

local function main()
  local start_time = os.clock()

  local count = 0

  for at = 2, 9999 do
    if is_prime(at) then
      count = count + 1
    end
  end

  print(count)

  print(os.clock() - start_time)
end

main()
