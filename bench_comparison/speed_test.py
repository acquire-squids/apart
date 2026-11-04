import time

def main():
    start_time = time.perf_counter()

    x = 1_000_000

    while x > 0:
        x -= 1

    print(time.perf_counter() - start_time)

main()
