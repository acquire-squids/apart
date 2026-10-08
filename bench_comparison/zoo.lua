local function zoo_new()
  local zoo = {
    aardvark = 1,
    baboon = 1,
    cat = 1,
    donkey = 1,
    elephant = 1,
    fox = 1,
  }

  zoo.ant = function(zoo)
    return zoo.aardvark
  end

  zoo.banana = function(zoo)
    return zoo.baboon
  end

  zoo.tuna = function(zoo)
    return zoo.cat
  end

  zoo.hay = function(zoo)
    return zoo.donkey
  end

  zoo.grass = function(zoo)
    return zoo.elephant
  end

  zoo.mouse = function(zoo)
    return zoo.fox
  end

  return zoo
end

local function main()
  local start_time = os.clock()

  local zoo, total = zoo_new(), 0

  while total < 100000000 do
    total = total
      + zoo.ant(zoo)
      + zoo.banana(zoo)
      + zoo.tuna(zoo)
      + zoo.hay(zoo)
      + zoo.grass(zoo)
      + zoo.mouse(zoo)
  end

  print(total)

  -- 1.425 seconds Lua 5.4.6
  print(os.clock() - start_time)
end

main()
