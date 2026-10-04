"""Generators and asynchronous functions.

`yield`, `async def`, `await` and asynchronous iteration have no counterpart in
most targets, so each has to be detected rather than approximated.
"""


def counter(limit):
    i = 0
    while i < limit:
        yield i
        i = i + 1


def delegating(items):
    yield from items


def paired(rows):
    for row in rows:
        yield len(row), row


async def fetch(client, url):
    response = await client.get(url)
    return response


async def gather_all(client, urls):
    results = []
    for url in urls:
        results.append(await fetch(client, url))
    return results


async def streaming(source):
    async for item in source:
        print(item)
