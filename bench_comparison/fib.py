import time

def fib(n):
    if n <= 1:
        return n

    return fib(n - 2) + fib(n - 1)

def main():
    start_time = time.perf_counter()

    n = 35
    
    print(fib(n))

    # 0.757 seconds Python 3.12.3
    print(time.perf_counter() - start_time)

main()
