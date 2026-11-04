import time

class Zoo:
    def __init__(self):
        self.aardvark = 1
        self.baboon = 1
        self.cat = 1
        self.donkey = 1
        self.elephant = 1
        self.fox = 1

    def ant(self):
        return self.aardvark

    def banana(self):
        return self.baboon

    def tuna(self):
        return self.cat

    def hay(self):
        return self.donkey

    def grass(self):
        return self.elephant

    def mouse(self):
        return self.fox

def main():
    start_time = time.perf_counter()

    zoo = Zoo()
    total = 0

    while total < 100_000_000:
        total += (zoo.ant()
            + zoo.banana()
            + zoo.tuna()
            + zoo.hay()
            + zoo.grass()
            + zoo.mouse())

    print(total)

    print(time.perf_counter() - start_time)

main()
