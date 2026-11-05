Host {{ op://ChainArgos/ojp2npvge6lsk3ywardbt2b5xq/ssh_hosts }}
  HostName {{ op://ChainArgos/ojp2npvge6lsk3ywardbt2b5xq/ipv4 }}
  User {{ op://ChainArgos/ojp2npvge6lsk3ywardbt2b5xq/username }}

Host {{ op://ChainArgos/66inoeodyhkh7ep7tanxmhx3i4/ssh_hosts }}
  HostName {{ op://ChainArgos/66inoeodyhkh7ep7tanxmhx3i4/ipv4 }}
  User {{ op://ChainArgos/66inoeodyhkh7ep7tanxmhx3i4/username }}

Host {{ op://ChainArgos/2ugnfozuxr5l3gr6cvit2pl75a/ssh_hosts }}
  HostName {{ op://ChainArgos/2ugnfozuxr5l3gr6cvit2pl75a/ipv4 }}
  User {{ op://ChainArgos/2ugnfozuxr5l3gr6cvit2pl75a/username }}

Host {{ op://ChainArgos/2i5t645vcqszemoqvl6wglsr4u/ssh_hosts }}
  HostName {{ op://ChainArgos/2i5t645vcqszemoqvl6wglsr4u/ipv4 }}
  User {{ op://ChainArgos/2i5t645vcqszemoqvl6wglsr4u/username }}

Host {{ op://ChainArgos/6dyeb2uc45222gtbvagmis2zf4/ssh_hosts }}
  HostName {{ op://ChainArgos/6dyeb2uc45222gtbvagmis2zf4/ipv4 }}
  User {{ op://ChainArgos/6dyeb2uc45222gtbvagmis2zf4/username }}

Host {{ op://ChainArgos/pz5dtmva7boxetcexim2mdtjcm/ssh_hosts }}
  HostName {{ op://ChainArgos/pz5dtmva7boxetcexim2mdtjcm/ipv4 }}
  User {{ op://ChainArgos/pz5dtmva7boxetcexim2mdtjcm/username }}

Host {{ op://ChainArgos/hbf4yoh4fhnwp5drdfnbcf6br4/ssh_hosts }}
  HostName {{ op://ChainArgos/hbf4yoh4fhnwp5drdfnbcf6br4/ipv4 }}
  User {{ op://ChainArgos/hbf4yoh4fhnwp5drdfnbcf6br4/username }}

Host {{ op://ChainArgos/k75f2e6gatyydaouizuel33jji/ssh_hosts }}
  HostName {{ op://ChainArgos/k75f2e6gatyydaouizuel33jji/ipv4 }}
  User {{ op://ChainArgos/k75f2e6gatyydaouizuel33jji/username }}

Host {{ op://ChainArgos/i4dsp3e5b67yb2zgbarz3nac44/ssh_hosts }}
  HostName {{ op://ChainArgos/i4dsp3e5b67yb2zgbarz3nac44/ipv4 }}
  User {{ op://ChainArgos/i4dsp3e5b67yb2zgbarz3nac44/username }}

Host {{ op://ChainArgos/ojp2npvge6lsk3ywardbt2b5xq/ssh_hosts }} {{ op://ChainArgos/66inoeodyhkh7ep7tanxmhx3i4/ssh_hosts }} {{ op://ChainArgos/2ugnfozuxr5l3gr6cvit2pl75a/ssh_hosts }} {{ op://ChainArgos/2i5t645vcqszemoqvl6wglsr4u/ssh_hosts }} {{ op://ChainArgos/6dyeb2uc45222gtbvagmis2zf4/ssh_hosts }} {{ op://ChainArgos/pz5dtmva7boxetcexim2mdtjcm/ssh_hosts }} {{ op://ChainArgos/hbf4yoh4fhnwp5drdfnbcf6br4/ssh_hosts }} {{ op://ChainArgos/k75f2e6gatyydaouizuel33jji/ssh_hosts }} {{ op://ChainArgos/i4dsp3e5b67yb2zgbarz3nac44/ssh_hosts }}
  ForwardAgent no
  ForwardX11 no
  StrictHostKeyChecking accept-new
  SetEnv TERM=xterm-256color
  ServerAliveInterval 30
  ServerAliveCountMax 3
  TCPKeepAlive yes
  ControlMaster auto
  ControlPath ~/.ssh/cm-%C
  ControlPersist 10m
