// Client
// Connects to a given server
// The most basic functions the client should supply
// The flow diagram goes inbound(1,2,3) then outbound(3,2,1)
/* | INBOUND                     | OUTBOUND
-------------------------------------------
 1 | player input -> logic       |
 2 | packets -> logic            | logic -> packets
 3 | logic -> audiovisual output |
*/

struct Client {}
