import time

def is_prime(n):
    if n < 2 or (n != 2 and n % 2 == 0):
        return False

    for i in range(3, n, 2):
        if n % i == 0:
            return False

    return True

def main():
    start_time = time.perf_counter()

    count = 0

    for at in range(2, 10_000):
        if is_prime(at):
            count += 1
    
    print(count)

    print(time.perf_counter() - start_time)

main()
