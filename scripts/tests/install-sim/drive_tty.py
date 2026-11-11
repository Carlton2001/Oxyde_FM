import os, pty, sys, time, select, fcntl, termios, struct
# SIM_COLUMNS sets the terminal width (100).
# usage: drive_tty.py keys(comma: E=enter D=down U=up) cmd...
keys=sys.argv[1].split(',') if sys.argv[1] else []
pid, fd = pty.fork()
if pid==0:
    os.execvp(sys.argv[2], sys.argv[2:])
cols=int(os.environ.get('SIM_COLUMNS','100'))
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 40, cols, 0, 0))
out=b''
m={'E':b'\r','D':b'\x1b[B','U':b'\x1b[A','Q':b'q'}
def pump(t):
    global out
    end=time.time()+t
    while time.time()<end:
        r,_,_=select.select([fd],[],[],0.05)
        if r:
            try: d=os.read(fd,65536)
            except OSError: return False
            if not d: return False
            out+=d
    return True
pump(1.5)
for k in keys:
    os.write(fd,m[k]); pump(0.6)
pump(2)
sys.stdout.buffer.write(out)
