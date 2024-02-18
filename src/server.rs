
// Server
// Hosts a game and allows clients to conenct
// The most basic functions the server should supply
// The flow diagram goes inbound(1,2,3) then outbound(3,2,1)
/* | INBOUND                     | OUTBOUND
-------------------------------------------
 1 | packets -> logic            | logic -> packets
 2 | logic -> simulate world     | 
*/
