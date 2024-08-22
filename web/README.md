# teetty-web

This is an experiment that streams a terminal app into a browser so you can
monitor it remotely.  It's a small flask app that uses asciinema's player
against a stream captured by teetty.

Step 1: run a program

```
./bin/monitor-remotely python
```

Step 2: start the server

```
./bin/teetty-web-server
```

Step 3: go to localhost:5000 and see stuff show up.
