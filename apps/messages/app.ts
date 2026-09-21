import type { Answer, Sources, Result } from './app.contract.d.ts';
import { MessagesReplica, nativeCore, grants as replicaGrants, type Records } from './snapback-client';
import { loadBrowserDevice } from './snapback-core';
export const appId = 'com.exact.messages';
export const grants = replicaGrants;
type Message = Result<'conversation'>['messages'][number];
type StoredMessage = Pick<Message,'id'|'body'|'outgoing'|'time'|'day'|'delivery'|'sender'|'reply'|'replyRoot'> & { second:number; order:number; reactions:Record<string,string> };
type Person = Result<'inbox'>['people'][number];
const people: (Omit<Person,'draft'|'reply'|'muted'> & {address?:string})[] = [
  {id:'maya',address:'+14155550101',name:'Maya Chen',initials:'MC',color:'#a58ac4',preview:'See you there! ☕️',time:'9:41 AM',unread:true},
  {id:'weekend',name:'Weekend people',initials:'☀',color:'#e2a34d',preview:'Jules: I’ll bring sandwiches 🥪',time:'9:33 AM',unread:true},
  {id:'dad',address:'+14155550102',name:'Dad',initials:'D',color:'#829baa',preview:'Loved seeing you this weekend ❤️',time:'Yesterday',unread:false},
  {id:'alex',address:'+14155550103',name:'Alex Rivera',initials:'AR',color:'#76a998',preview:'You sent a photo',time:'Yesterday',unread:false},
  {id:'jules',address:'+14155550104',name:'Jules',initials:'J',color:'#cd8f99',preview:'That sounds perfect',time:'Monday',unread:false},
  {id:'sam',address:'+14155550105',name:'Sam',initials:'S',color:'#8196c0',preview:'Thanks again!',time:'Monday',unread:false},
];
// The array owns presentation order; the index keeps live contact references
// and stable stored positions when contacts are prepended or appended.
const personIndex=new Map(people.map((person,position)=>[person.id,{person,position}]));
let firstPersonPosition=0,lastPersonPosition=people.length-1,rebasedPeople=false;
function insertPerson(person:typeof people[number],front:boolean):void {
  const edge=front?firstPersonPosition:lastPersonPosition;
  let position=edge+(front?-1:1);
  if(!Number.isFinite(position)||(front?position>=edge:position<=edge)){
    // Imported finite positions can exhaust Number's adjacent range. Rebase as
    // one ordinary atomic edit; the existing record cap still applies.
    people.forEach((p,i)=>personIndex.get(p.id)!.position=i);
    firstPersonPosition=0;lastPersonPosition=people.length-1;rebasedPeople=true;
    position=front?-1:people.length;
  }
  personIndex.set(person.id,{person,position});
  if(front){firstPersonPosition=position;people.unshift(person);}
  else{lastPersonPosition=position;people.push(person);}
}
const reactions = [
  {id:'heart',value:'❤️',label:'Love'}, {id:'like',value:'👍',label:'Like'},
  {id:'dislike',value:'👎',label:'Dislike'}, {id:'laugh',value:'haha',label:'Laugh'},
  {id:'emphasis',value:'‼️',label:'Emphasize'}, {id:'question',value:'❓',label:'Question'},
  {id:'laugh-face',value:'😂',label:'Laughing face'},
  {id:'smile',value:'😊',label:'Smile'}, {id:'fire',value:'🔥',label:'Fire'},
  {id:'party',value:'🎉',label:'Celebrate'}, {id:'eyes',value:'👀',label:'Eyes'},
  {id:'hundred',value:'💯',label:'One hundred'}, {id:'thanks',value:'🙏',label:'Thank you'},
];
const threads = new Map<string, StoredMessage[]>();
type ThreadIndex = {
  byId:Map<string,StoredMessage>;
  awaitingRead:Set<StoredMessage>;
  replyCounts:Map<string,number>;
  replies:Map<string,StoredMessage[]>;
  lastOutgoing:StoredMessage|undefined;
  replyOutgoing:Map<string,StoredMessage>;
};
const emptyIndex=():ThreadIndex=>({byId:new Map(),awaitingRead:new Set(),replyCounts:new Map(),replies:new Map(),lastOutgoing:undefined,replyOutgoing:new Map()});
const indexes = new Map<string,ThreadIndex>();
const windowSize = 200;
type Position = Pick<StoredMessage,'order'|'id'>;
const compareMessages=(a:Position,b:Position)=>a.order-b.order || (a.id<b.id?-1:a.id>b.id?1:0);
const positionCursor=(row:StoredMessage|undefined)=>row?`${row.order}:${row.id}`:'';
function cursorPosition(cursor:string):Position {
  const separator=cursor.indexOf(':'),order=cursor.slice(0,separator),id=cursor.slice(separator+1);
  if(separator<1 || !id || !/^[0-9]+$/.test(order) || !Number.isSafeInteger(Number(order)))throw new Error('Invalid conversation cursor');
  return {order:Number(order),id};
}
function lowerBound(rows:StoredMessage[],position:Position) {
  let low=0,high=rows.length;
  while(low<high){const mid=Math.floor((low+high)/2);if(compareMessages(rows[mid],position)<0)low=mid+1;else high=mid;}
  return low;
}
function cursorAnchor(rows:StoredMessage[],cursor:string) {
  const position=cursorPosition(cursor),at=lowerBound(rows,position);
  // Resolve deleted anchors backward so later arrivals cannot capture them.
  return at<rows.length && compareMessages(rows[at],position)===0?at:Math.max(0,at-1);
}
function insertSorted(rows:StoredMessage[],row:StoredMessage) {
  if(!rows.length || compareMessages(rows[rows.length-1],row)<0)rows.push(row);
  else rows.splice(lowerBound(rows,row),0,row);
}
// All membership changes pass through these helpers. The durable rows retain
// exactly their old shape; these indexes are rebuilt, never persisted.
function insertMessage(id:string,row:StoredMessage) {
  if(!threads.has(id))replaceThread(id,[]);
  const rows=threads.get(id)!,index=indexes.get(id)!;
  if(index.byId.has(row.id))return;
  insertSorted(rows,row);index.byId.set(row.id,row);
  const replies=index.replies.get(row.replyRoot)||[];
  insertSorted(replies,row);index.replies.set(row.replyRoot,replies);
  if(row.id!==row.replyRoot)index.replyCounts.set(row.replyRoot,(index.replyCounts.get(row.replyRoot)||0)+1);
  if(row.outgoing){
    if(row.delivery!=='Read')index.awaitingRead.add(row);
    if(!index.lastOutgoing || compareMessages(index.lastOutgoing,row)<0)index.lastOutgoing=row;
    const previous=index.replyOutgoing.get(row.replyRoot);
    if(!previous || compareMessages(previous,row)<0)index.replyOutgoing.set(row.replyRoot,row);
  }
}
function replaceThread(id:string,rows:StoredMessage[]) {
  threads.set(id,[]);
  indexes.set(id,emptyIndex());
  for(const row of rows)insertMessage(id,row);
}
function removeMessages(id:string,selected:Set<string>) {
  const removed:StoredMessage[]=[],kept:StoredMessage[]=[];
  for(const row of threads.get(id)||[])(selected.has(row.id)?removed:kept).push(row);
  if(removed.length)replaceThread(id,kept);
  return removed;
}
const muted = new Set<string>();
const blocked = new Set<string>();
const localContacts = new Map<string,{first:string,last:string,company:string,phone:string,email:string,notes:string}>();
const deleted = new Set<string>();
const recoverable = new Map<string,{rows:{message:StoredMessage,expires:number}[],expires:number}>();
const recoveryDay = 86400000;
// Lower bound for every archived row's expiry. Removing rows may leave it early;
// archive and restore include new deadlines, and an actual scan tightens it.
let nextRecoveryExpiry=Infinity;
let messageOrder = 0;
const drafts = new Map<string, {draft:string,reply:string}>();
let revision = 0;
let namespace='';
const pending = new Map<string, {start:number,end:number,reply:string}>();
let ticks = 0;
// Conservative lower bound for the next receipt/reply on a forward clock.
// Removing or replacing schedules may leave it early, never late. A real scan
// tightens it again; rewinds always scan so previously crossed receipts re-arm.
let nextPendingTime=Infinity;
function idleReplyTick(now:number):boolean {
  if(now>=ticks && now<nextPendingTime){ticks=now;return true;}
  return false;
}
const changed = () => ({revision,pending:pending.size>0});
const scrollRevisions = new Map(people.map((person,index)=>[person.id,index]));
let scrollGeneration = people.length;
// Fixture dates stay fixed; precise within-day times decide bubble runs.
const fixtureStart = 9*3600+42*60;
function clockSecond(time:string) {
  const [,hour,minute,period]=/^(\d+):(\d+) (AM|PM)$/.exec(time)!;
  return (Number(hour)%12+(period==='PM'?12:0))*3600+Number(minute)*60;
}
function fixtureTime(nowMs:number) {
  const second=fixtureStart+nowMs/1000,minute=Math.floor(second/60),hour=Math.floor(minute/60)%24;
  return {second,time:`${hour%12 || 12}:${String(minute%60).padStart(2,'0')} ${hour<12?'AM':'PM'}`};
}
function sameRun(a:StoredMessage|undefined,b:StoredMessage|undefined) {
  return !!a && !!b && a.sender===b.sender && a.outgoing===b.outgoing && a.day===b.day && b.second>=a.second && b.second-a.second<60;
}
function message(id:string,body:string,outgoing:boolean,time:string,sender=outgoing?'me':'',day='Today',second=clockSecond(time)): StoredMessage {
  return {second,order:messageOrder++,id,body,outgoing,time,day,delivery:outgoing?'Read':'',sender,reactions:{},reply:'',replyRoot:id};
}
function expireDeleted(now:number) {
  if(now<nextRecoveryExpiry)return;
  nextRecoveryExpiry=Infinity;
  for(const [id,archive] of recoverable) {
    let expires=Infinity;
    const kept=archive.rows.filter(r=>{
      if(!(r.expires>now))return false;
      expires=Math.min(expires,r.expires);return true;
    });
    if(kept.length){recoverable.set(id,{rows:kept,expires});nextRecoveryExpiry=Math.min(nextRecoveryExpiry,expires);}else recoverable.delete(id);
  }
}
function archiveMessages(id:string,rows:StoredMessage[],now:number) {
  expireDeleted(now);
  if(rows.length){
    const expires=now+30*recoveryDay;
    const archive=recoverable.get(id)||{rows:[],expires:Infinity};
    for(const message of rows)archive.rows.push({message,expires});
    archive.expires=Math.min(archive.expires,expires);
    recoverable.set(id,archive);
    nextRecoveryExpiry=Math.min(nextRecoveryExpiry,expires);
  }
}
function refreshPreview(id:string) {
  const person=personIndex.get(id)?.person,rows=threads.get(id),last=rows?.[rows.length-1];
  if(person){person.preview=last?.body || '';person.time=last?(last.day==='Today'?last.time:last.day):'';}
}
function replyTo(item:StoredMessage,id:string,root:string) {
  if(!root) return;
  const index=indexes.get(id)!,parent=index.byId.get(root);
  // A thread keeps its identity when its original bubble is deleted.
  item.replyRoot=parent?.replyRoot || root;
  item.reply=parent?.body || index.replies.get(root)?.[0]?.reply || '';
}
replaceThread('maya',[
  message('m1','Hey! Are you around this morning?',false,'9:30 AM'),
  message('m2','Yeah! Just finishing a few things',true,'9:31 AM'),
  message('m3','Want to grab coffee? There’s a new place on Valencia I’ve been wanting to try',false,'9:32 AM'),
  message('m4','The one with the little green door?',true,'9:33 AM'),
  message('m5','Yes!! That’s the one',false,'9:34 AM'),
  message('m6','I walked past it yesterday. It looks so good',false,'9:34 AM'),
  message('m7','I’m in. Meet you there at 10?',true,'9:36 AM'),
  message('m8','Perfect 😊',false,'9:37 AM'),
  message('m9','I’ll get us a table',true,'9:40 AM'),
  message('m10','See you there! ☕️',false,'9:41 AM'),
]);
threads.get('maya')![6].reactions.maya='❤️';
for(const person of people.slice(2)) replaceThread(person.id,[
  message(`${person.id}-1`,'Hey! How’s your week going?',false,'9:20 AM','',person.time),
  message(`${person.id}-2`,'Really good! How about yours?',true,'9:24 AM','me',person.time),
  message(`${person.id}-3`,person.preview,false,'9:32 AM','',person.time),
]);
replaceThread('weekend',[
  message('weekend-1','Anyone up for a hike on Saturday?',false,'9:20 AM','maya'),
  message('weekend-2','Definitely! Count me in 🌲',true,'9:24 AM'),
  message('weekend-3','Who’s bringing snacks?',false,'9:32 AM','alex'),
  message('weekend-4','I can bring fruit.',false,'9:32 AM','alex'),
  message('weekend-5','I’ll bring sandwiches 🥪',false,'9:33 AM','jules'),
]);
// Member sets give a new compose session the same local thread in any order.
const groups = new Map<string,string[]>([['weekend',['alex','jules','maya']]]);
// Local address identity only: the example never looks up service availability.
function addressPerson(value:string):typeof people[number] | undefined {
  const text=value.trim();
  let address='',name='';
  if(/^\+?[0-9() .-]+$/.test(text)) {
    const digits=text.replace(/\D/g,'');
    if(digits.length<3 || digits.length>15) return;
    address=text.startsWith('+')?`+${digits}`:digits.length===10?`+1${digits}`:digits.length===11 && digits[0]==='1'?`+${digits}`:digits;
    name=/^\+1\d{10}$/.test(address)?`+1 (${address.slice(2,5)}) ${address.slice(5,8)}-${address.slice(8)}`:address;
  } else if(/^[^\s@,;|<>]+@[^\s@,;|<>]+\.[^\s@,;|<>]+$/.test(text)) {
    address=text.toLowerCase();name=address;
  } else return;
  return people.find(p=>p.address===address) || {id:`address:${encodeURIComponent(address)}`,address,name,initials:'',color:'#829baa',preview:'',time:'Now',unread:false};
}
function recipientById(id:string) {
  const existing=personIndex.get(id)?.person;
  if(existing) return existing.address?existing:undefined;
  if(!id.startsWith('address:')) return;
  try {
    const person=addressPerson(decodeURIComponent(id.slice(8)));
    if(person?.id===id) return person;
  } catch { /* An invalid encoded address is not a recipient. */ }
}
function selectedPeople(ids:string) {
  return [...new Set(ids.split('|'))].filter(id=>!groups.has(id)).map(recipientById).filter((p):p is typeof people[number]=>!!p);
}
function recipientTarget(selected:typeof people) {
  const ids=selected.map(p=>p.id).sort();
  if(ids.length<2) return ids[0] || '';
  return [...groups].find(([,members])=>members.join('|')===ids.join('|'))?.[0] || `group:${ids.join('|')}`;
}
function ensureConversation(id:string) {
  if(threads.has(id)) return;
  const address=recipientById(id);
  if(address) {
    if(!personIndex.has(id)) insertPerson(address,true);
    replaceThread(id,[]);
    return;
  }
  if(!id.startsWith('group:')) return;
  const members=selectedPeople(id.slice(6));
  if(members.length<2 || recipientTarget(members)!==id) return;
  groups.set(id,members.map(p=>p.id).sort());
  insertPerson({id,name:members.map(p=>p.name.split(' ')[0]).join(', '),initials:members.slice(0,2).map(p=>p.initials[0]).join(''),color:'#829baa',preview:'',time:'Now',unread:false},true);
  replaceThread(id,[]);
}
function responder(id:string) {
  return recipientById(groups.get(id)?.[0] || id);
}
function conversation(id:string,replying:string,selection:string,cursor:string):Result<'conversation'> {
  const person=personIndex.get(id)?.person || people[0];
  const rows=threads.get(person.id) || [];
  const index=indexes.get(person.id)||emptyIndex();
  const anchor=cursor===''?rows.length-1:cursorAnchor(rows,cursor);
  const start=cursor===''?Math.max(0,rows.length-windowSize):Math.max(0,anchor-windowSize/2);
  const end=Math.min(rows.length,start+windowSize);
  const selectedRows=[...new Set(selection.split('|'))].flatMap(id=>{const row=index.byId.get(id);return row?[row]:[];});
  const selected=new Set(selectedRows.map(m=>m.id));
  const decorate=(visible:StoredMessage[],before:StoredMessage|undefined,after:StoredMessage|undefined,lastOutgoing:StoredMessage|undefined)=>{
    return visible.map((m,i)=>{
      const previous=i===0?before:visible[i-1],next=i===visible.length-1?after:visible[i+1];
      const sender=groups.has(person.id) && !m.outgoing?recipientById(m.sender):undefined;
      const startsDay=!previous || previous.day!==m.day;
      const {second:_second,order:_order,reactions:saved,...content}=m;
      const entries=Object.entries(saved).sort(([a],[b])=>a==='me'?-1:b==='me'?1:0).map(([id,value],i)=>{
        const who=id==='me'?{name:'You',initials:'ME',color:'#859bc1'}:recipientById(id);
        return {id,value,name:who?.name||id,initials:who?.initials||'',color:who?.color||'#829baa',own:id==='me',offset:i*27,edge:i===0};
      });
      const byValue=new Map<string,typeof entries>();
      for(const entry of entries){const group=byValue.get(entry.value)||[];group.push({...entry,offset:group.length*18});byValue.set(entry.value,group);}
      const reactionGroups=[...byValue].map(([value,people])=>({value,people,width:32+(people.length-1)*18}));
      return {...content,reaction:saved.me||'',reactionCount:entries.length,reactionEntries:entries,reactionGroups,reactionPanelWidth:Math.max(124,18+reactionGroups.length*98),chosen:selected.has(m.id),selection:(selected.has(m.id)?[...selected].filter(id=>id!==m.id):[...selected,m.id]).join("|"),timeLabel:startsDay?`${m.day} ${m.time}`:'',delivery:m===lastOutgoing?m.delivery:'',
        tail:!sameRun(m,next),
        senderName:sender?.name || '',senderInitials:sender?.initials || '',senderColor:sender?.color || '',
        showSender:!!sender && (startsDay || previous!.sender!==m.sender),
        replyCount:index.replyCounts.get(m.id)||0};
    });
  };
  const messages=decorate(rows.slice(start,end),rows[start-1],rows[end],index.lastOutgoing);
  const activity=pending.get(person.id);
  const typing=!!activity && ticks>=activity.start;
  return {selectedText:selectedRows.sort(compareMessages).map(m=>m.body).join("\n"),selectionCount:selected.size,typingName:typing?(responder(person.id)?.name || ''):'',
    typingAvatar:typing && groups.has(person.id)?responder(person.id)!.initials:'',typingRoot:typing?activity!.reply:'',
    id:person.id,name:person.name,initials:person.initials,color:person.color,reactions,
    muted:muted.has(person.id),blocked:blocked.has(person.id),knownContact:!person.id.startsWith('address:') || localContacts.has(person.id),
    contactKind:person.address?.includes('@')?'email':'phone',
    contactAddress:person.address && /^\+1\d{10}$/.test(person.address)?`+1 (${person.address.slice(2,5)}) ${person.address.slice(5,8)}-${person.address.slice(8)}`:person.address || '',
    messages,earlier:positionCursor(rows[start]),later:positionCursor(rows[end-1]),hasEarlier:start>0,hasLater:end<rows.length,
    replies:decorate(index.replies.get(replying)||[],undefined,undefined,index.replyOutgoing.get(replying)),revision,scrollRevision:scrollRevisions.get(person.id)||0};
}
const sources: Sources = {
  syncMessages: () => changed(),
  syncState: () => replica?.status() || 'Conversation preview',
  inbox: ([query,_revision])=>({people:people.filter(p=>threads.has(p.id) && !deleted.has(p.id) && p.name.toLowerCase().includes(query.toLowerCase())).map(({address:_address,...p})=>({...p,muted:muted.has(p.id),...(drafts.get(p.id)||{draft:'',reply:''})}))}),
  recentlyDeleted: ([selection,_revision,now])=>{
    expireDeleted(now);
    const ids=new Set(selection.split('|'));
    const rows=people.flatMap(p=>{
      const archive=recoverable.get(p.id);if(!archive?.rows.length)return [];
      return [{id:p.id,name:p.name,initials:p.initials,color:p.color,count:archive.rows.length,
        days:Math.ceil((archive.expires-now)/recoveryDay),chosen:ids.has(p.id),
        selection:(ids.has(p.id)?[...ids].filter(id=>id!==p.id):[...ids,p.id]).filter(Boolean).join('|')}];
    });
    const chosen=selection?rows.filter(p=>p.chosen):rows;
    return {people:rows,targets:chosen.map(p=>p.id).join('|'),count:chosen.reduce((n,p)=>n+p.count,0)};
  },
  recipients: ([ids,query,body,_revision])=>{
    const selected=selectedPeople(ids),selectedIds=new Set(selected.map(p=>p.id)),text=query.trim();
    const pending=text?(people.find(p=>p.address && p.name.toLowerCase()===text.toLowerCase()) || addressPerson(text)):undefined;
    const resolved=pending?[...selectedIds,...(selectedIds.has(pending.id)?[]:[pending.id])].join('|'):'';
    const matches=people.filter(p=>p.address && !selectedIds.has(p.id) && p.name.toLowerCase().includes(text.toLowerCase()));
    if(pending && !selectedIds.has(pending.id) && !matches.some(p=>p.id===pending.id)) matches.unshift(pending);
    return {selected:selected.map(p=>({id:p.id,name:p.name,without:selected.filter(other=>other.id!==p.id).map(p=>p.id).join('|')})),
      people:matches.map(({address:_address,...p})=>({...p,draft:'',reply:'',muted:false})),resolved,
      last:selected[selected.length-1]?.id || '',withoutLast:selected.slice(0,-1).map(p=>p.id).join('|'),
      target:recipientTarget(resolved?selectedPeople(resolved):selected),canSend:(selected.length>0 || !!pending) && (!text || !!pending) && !!body.trim()};
  },
  conversationDraft: ([id,_revision])=>({thread:id,...(drafts.get(id)||{draft:'',reply:''})}),
  conversation: ([id,_revision,replying,selection,cursor])=>conversation(id,replying,selection,cursor),
  markRead: ([id])=>{const p=personIndex.get(id)?.person;if(p)p.unread=false;revision++;return changed();},
  setConversationUnread: ([id,unread])=>{
    const person=deleted.has(id)?undefined:personIndex.get(id)?.person;
    if(person && person.unread!==unread){person.unread=unread;revision++;}
    return changed();
  },
  muteConversation: ([id])=>{
    if(threads.has(id)){if(muted.has(id))muted.delete(id);else muted.add(id);revision++;}
    return changed();
  },
  blockConversation: ([id,value])=>{
    if(threads.has(id) && !groups.has(id)) {
      if(value){blocked.add(id);pending.delete(id);}else blocked.delete(id);
      revision++;
    }
    return changed();
  },
  createLocalContact: ([first,last,company,phone,email,notes])=>{
    const name=[first.trim(),last.trim()].filter(Boolean).join(' ') || company.trim();
    const initials=[first.trim(),last.trim()].filter(Boolean).map(v=>[...v][0]).join('').toUpperCase() || [...company.trim()].slice(0,2).join('').toUpperCase();
    const addresses=[phone,email].map(addressPerson).filter((p):p is typeof people[number]=>!!p);
    if(!addresses.length && name) addresses.push({id:`contact:${namespace}${++revision}`,name,initials,color:'#92a8ce',preview:'',time:'Now',unread:false});
    for(const candidate of addresses) {
      let person=personIndex.get(candidate.id)?.person;
      if(!person){person=candidate;insertPerson(person,false);}
      if(name)person.name=name;
      person.initials=initials;
      localContacts.set(person.id,{first,last,company,phone,email,notes});
    }
    revision++;
    return changed();
  },
  deleteConversation: ([id,now])=>{
    const rows=threads.get(id);
    if(rows){archiveMessages(id,rows,now);replaceThread(id,[]);deleted.add(id);pending.delete(id);drafts.delete(id);revision++;}
    return changed();
  },
  recoverConversations: ([selection,now])=>{
    expireDeleted(now);
    for(const id of new Set(selection.split('|'))) {
      const records=recoverable.get(id)?.rows;if(!records?.length)continue;
      for(const row of records)insertMessage(id,row.message);
      recoverable.delete(id);deleted.delete(id);refreshPreview(id);revision++;
    }
    return changed();
  },
  purgeConversations: ([selection,now])=>{
    expireDeleted(now);
    for(const id of new Set(selection.split('|')))if(recoverable.delete(id))revision++;
    return changed();
  },
  saveDraft: ([id,draft,reply])=>{
    if(threads.has(id)) {
      if(draft || reply) drafts.set(id,{draft,reply}); else drafts.delete(id);
      revision++;
    }
    return changed();
  },
  sendMessage: ([id,body,reply,now,nowMs])=>{
    if(body.trim()) {
      ensureConversation(id);
      deleted.delete(id);
    }
    const rows=threads.get(id),person=personIndex.get(id)?.person;
    if(rows && person && body.trim()) {
      const at=fixtureTime(nowMs);
      const item=message(`sent-${namespace}${++revision}`,body,true,at.time,'me','Today',at.second);
      item.delivery='Delivered';
      replyTo(item,id,reply);
      insertMessage(id,item);person.preview=body;person.time=item.time;person.unread=false;
      drafts.delete(id);
      scrollRevisions.set(id,++scrollGeneration);
      if(!blocked.has(id)) {
        pending.set(id,{start:now+3,end:now+15,reply:reply?item.replyRoot:''});
        nextPendingTime=Math.min(nextPendingTime,now+3,now+15);
      }
    }
    return changed();
  },
  advanceReplies: ([now,activeThread,nowMs])=>{
    if(idleReplyTick(now))return changed();
    const previous=ticks;
    ticks=now;
    nextPendingTime=Infinity;
    for(const [id,activity] of pending) {
      if(previous<activity.start && now>=activity.start) {
        const awaiting=indexes.get(id)?.awaitingRead;
        if(awaiting){for(const item of awaiting)item.delivery='Read';awaiting.clear();}
        revision++;
      }
      if(now<activity.end) {
        nextPendingTime=Math.min(nextPendingTime,now<activity.start?activity.start:activity.end);
        continue;
      }
      const person=personIndex.get(id)?.person!;
      const body=id==='weekend'?'Sounds good! 🌲':id==='maya'?'See you soon! ☕️':'Sounds good 😊';
      const sender=groups.has(id)?responder(id):undefined;
      const at=fixtureTime(nowMs);
      const item=message(`received-${namespace}${++revision}`,body,false,at.time,sender?.id || '','Today',at.second);
      replyTo(item,id,activity.reply);
      insertMessage(id,item);person.preview=sender?`${sender.name.split(' ')[0]}: ${body}`:body;person.time=item.time;person.unread=id!==activeThread;
      pending.delete(id);
      // Incoming activity follows only an already-pinned reader; no scroll command.
    }
    return changed();
  },
  react: ([id,messageId,emoji])=>{
    const m=indexes.get(id)?.byId.get(messageId);
    if(m){if(m.reactions.me===emoji)delete m.reactions.me;else m.reactions.me=emoji;revision++;}
    return changed();
  },
  deleteMessages: ([id,selection,now])=>{
    const selected=new Set(selection.split('|')),rows=threads.get(id);
    if(rows){
      const removed=removeMessages(id,selected);
      if(removed.length){
        archiveMessages(id,removed,now);
        revision++;
        refreshPreview(id);
      }
    }
    return changed();
  },
};
// Persistence contains authored data, never bubble geometry or selection state.
function snapshot():Records {
  const records:Records=new Map();
  people.forEach(person=>putPerson(records,person));
  for(const [id,rows] of threads)for(const message of rows)putMessage(records,id,message,null);
  for(const [id,archive] of recoverable)for(const row of archive.rows)putMessage(records,id,row.message,row.expires);
  // persist detaches changed values before awaiting storage; unchanged rows
  // already have an owned copy in the replica.
  return records;
}
function putPerson(records:Records,person:typeof people[number]):void {
  const id=person.id;
  records.set(`person:${id}`,{kind:'person',person,position:personIndex.get(id)!.position,conversation:threads.has(id),
    muted:muted.has(id),blocked:blocked.has(id),deleted:deleted.has(id),
    draft:drafts.get(id)||null,group:groups.get(id)||null,contact:localContacts.get(id)||null});
}
function putMessage(records:Records,conversation:string,message:StoredMessage,expires:number|null):void {
  records.set(`message:${encodeURIComponent(conversation)}:${encodeURIComponent(message.id)}`,{kind:'message',conversation,message,expires});
}
// Declare each durable source's footprint before running it. Unknown sources
// refuse before mutation, rather than silently omitting a newly authored edit.
// Returned nulls delete only keys that disappeared from this footprint.
function editRecords(source:string,args:readonly unknown[]):()=>Records {
  rebasedPeople=false;
  const id=String(args[0]);
  const person=(records:Records,key:string)=>{
    const value=personIndex.get(key);
    if(value)putPerson(records,value.person);
  };
  const live=(records:Records,key:string,ids:Iterable<string>)=>{
    const index=indexes.get(key);
    for(const messageId of ids){const row=index?.byId.get(messageId);if(row)putMessage(records,key,row,null);}
  };
  let capture:()=>Records,removes=false;
  switch(source){
    case 'markRead':case 'setConversationUnread':case 'muteConversation':case 'blockConversation':case 'saveDraft':
      capture=()=>{const rows:Records=new Map();person(rows,id);return rows;};break;
    case 'react':
      capture=()=>{const rows:Records=new Map();live(rows,id,[String(args[1])]);return rows;};break;
    case 'createLocalContact': {
      const ids=[String(args[3]),String(args[4])].map(addressPerson).filter((p):p is typeof people[number]=>!!p).map(p=>p.id);
      const previousLength=people.length;
      capture=()=>{
        const rows:Records=new Map();for(const key of ids)person(rows,key);
        // Name-only contacts allocate their ID inside the handler. All new
        // contacts append; include those rows without scanning older contacts.
        for(let i=previousLength;i<people.length;i++)putPerson(rows,people[i]);
        return rows;
      };break;
    }
    case 'sendMessage':
      capture=()=>{
        const rows:Records=new Map();
        person(rows,id);
        const messages=threads.get(id),last=messages?.[messages.length-1];
        if(last)putMessage(rows,id,last,null);
        return rows;
      };break;
    case 'advanceReplies': {
      const now=Number(args[0]);
      const due=[...pending].map(([key,activity])=>({key,receipt:ticks<activity.start && now>=activity.start,reply:now>=activity.end})).filter(row=>row.receipt||row.reply)
        // Capture row references before the handler marks them and clears the
        // derived index. Persistence reads their new values after the handler.
        .map(row=>({...row,receipts:row.receipt?[...(indexes.get(row.key)?.awaitingRead||[])]:[]}));
      capture=()=>{
        const rows:Records=new Map();
        for(const {key,receipts,reply} of due){
          person(rows,key);
          const messages=threads.get(key)||[];
          for(const row of receipts)putMessage(rows,key,row,null);
          const last=messages[messages.length-1];
          if(reply && last)putMessage(rows,key,last,null);
        }
        return rows;
      };break;
    }
    case 'recentlyDeleted':case 'purgeConversations':case 'deleteConversation':case 'deleteMessages':case 'recoverConversations': {
      const selected=new Map<string,Set<string>>(),persons=new Set<string>();
      const include=(key:string,messageId:string)=>{
        let ids=selected.get(key);if(!ids){ids=new Set();selected.set(key,ids);}ids.add(messageId);
      };
      const now=Number(args[source==='recentlyDeleted'||source==='deleteMessages'?2:1]);
      // Expiry can remove rows outside the selected conversation. Capture only
      // those candidates, retaining the handler's strict > comparison. Negating
      // < also scans for NaN rather than silently retaining expired rows.
      if(!(now<nextRecoveryExpiry))for(const [key,archive] of recoverable)for(const row of archive.rows)if(!(row.expires>now))include(key,row.message.id);
      if(source==='deleteConversation'){
        persons.add(id);for(const row of threads.get(id)||[])include(id,row.id);
      }else if(source==='deleteMessages'){
        persons.add(id);for(const key of String(args[1]).split('|'))include(id,key);
      }else if(source==='recoverConversations'||source==='purgeConversations'){
        for(const key of new Set(id.split('|'))){
          if(source==='recoverConversations')persons.add(key);
          for(const row of recoverable.get(key)?.rows||[])include(key,row.message.id);
        }
      }
      removes=true;capture=()=>{
        const rows:Records=new Map();
        for(const key of persons)person(rows,key);
        for(const [key,ids] of selected){
          live(rows,key,ids);
          // The same key moves between live and archived storage on delete or
          // recover. As in snapshot(), archived rows take precedence.
          for(const row of recoverable.get(key)?.rows||[])if(ids.has(row.message.id))putMessage(rows,key,row.message,row.expires);
        }
        return rows;
      };break;
    }
    default:throw new Error(`Messages source has no durable footprint: ${source}`);
  }
  const before=removes?capture():undefined;
  return ()=>{
    const after=capture();
    if(rebasedPeople)people.forEach(p=>putPerson(after,p));
    if(before)for(const key of before.keys())if(!after.has(key))after.set(key,null);
    return after;
  };
}
function restore(records:Records):void {
  type PersonRecord={kind:'person';person:typeof people[number];position:number;conversation:boolean;muted:boolean;blocked:boolean;deleted:boolean;draft:{draft:string;reply:string}|null;group:string[]|null;contact:typeof localContacts extends Map<string,infer C>?C:null};
  type MessageRecord={kind:'message';conversation:string;message:StoredMessage;expires:number|null};
  const persons:PersonRecord[]=[],messages:MessageRecord[]=[];
  for(const value of records.values()){
    if(!value || typeof value!=='object')throw new Error('Invalid Messages replica record');
    const row=JSON.parse(JSON.stringify(value)) as PersonRecord|MessageRecord;
    if(row.kind==='person'){
      if(!row.person || !['id','name','initials','color','preview','time'].every(k=>typeof (row.person as unknown as Record<string,unknown>)[k]==='string') || typeof row.person.unread!=='boolean' || !Number.isFinite(row.position))throw new Error('Invalid Messages contact record');
      persons.push(row);
    }else if(row.kind==='message'){
      if(!row.message || !['id','body','time','day','delivery','sender','reply','replyRoot'].every(k=>typeof (row.message as unknown as Record<string,unknown>)[k]==='string') || typeof row.message.outgoing!=='boolean' || !Number.isFinite(row.message.order) || !Number.isFinite(row.message.second) || !row.message.reactions || Object.values(row.message.reactions).some(v=>typeof v!=='string') || (row.expires!==null && !Number.isFinite(row.expires)))throw new Error('Invalid Messages message record');
      messages.push(row);
    }else throw new Error('Unknown Messages replica record');
  }
  people.splice(0);personIndex.clear();firstPersonPosition=0;lastPersonPosition=-1;rebasedPeople=false;threads.clear();indexes.clear();muted.clear();blocked.clear();deleted.clear();drafts.clear();groups.clear();localContacts.clear();recoverable.clear();nextRecoveryExpiry=Infinity;
  for(const row of persons.sort((a,b)=>a.position-b.position || a.person.id.localeCompare(b.person.id))){
    // Preserve the old find/position-map behavior for imported duplicate IDs:
    // lookups select the first person, while stored positions use the last row.
    const id=row.person.id;people.push(row.person);personIndex.set(id,{person:personIndex.get(id)?.person || row.person,position:row.position});
    firstPersonPosition=Math.min(firstPersonPosition,row.position);lastPersonPosition=Math.max(lastPersonPosition,row.position);
    if(row.conversation)replaceThread(id,[]);
    if(row.muted)muted.add(id);if(row.blocked)blocked.add(id);if(row.deleted)deleted.add(id);
    if(row.draft)drafts.set(id,row.draft);if(row.group)groups.set(id,row.group);if(row.contact)localContacts.set(id,row.contact);
  }
  for(const row of messages.sort((a,b)=>compareMessages(a.message,b.message))){
    if(row.expires!==null){
      const archive=recoverable.get(row.conversation)||{rows:[],expires:Infinity};
      archive.rows.push({message:row.message,expires:row.expires});archive.expires=Math.min(archive.expires,row.expires);
      recoverable.set(row.conversation,archive);nextRecoveryExpiry=Math.min(nextRecoveryExpiry,row.expires);
    }
    else insertMessage(row.conversation,row.message);
    messageOrder=Math.max(messageOrder,row.message.order+1);
  }
  for(const id of pending.keys())if(deleted.has(id)||blocked.has(id)||!threads.has(id))pending.delete(id);
  revision++;
}
let replica:MessagesReplica|undefined;
let configuredCore:ReturnType<typeof nativeCore>;
let opened=false;
let tail:Promise<unknown>=Promise.resolve();
function local<T>(work:()=>Promise<T>):Promise<T>{const result=tail.then(work);tail=result.catch(()=>{});return result;}
export const answer: Answer = (source,args,store,storage,native) => {
  // The bake and unconfigured unit-test host deliberately expose no storage;
  // the native hook records that external dependency before refusing it.
  if(!opened){configuredCore=nativeCore(native);opened=configuredCore!==null;}
  const core=configuredCore;
  if(core===null)return sources[source](args,store,storage,native);
  const ready=async()=>{
    if(configuredCore===null)return undefined;
    if(!replica){
      try{const client=await MessagesReplica.open(storage,core);namespace=client.namespace;await client.seed(snapshot());restore(client.initial());replica=client;}
      catch(error){
        if((error instanceof Error?error.message:String(error))!=='storage is unavailable in agent mode')throw error;
        configuredCore=null;opened=true;return undefined;
      }
    }
    return replica;
  };
  const run=async()=>{
  // Network awaits never hold the local action queue. Only applying a received
  // page and replacing the model enter the same short gate as local edits.
  if(source==='syncMessages')return local(ready).then(async client=>{if(!client)return changed();const previous=client.status();await client.sync(Number(args[0]),local,restore);if(previous!==client.status())revision++;return changed();});
  return local(async()=>{
    const client=await ready();
    if(!client)return sources[source](args,store,storage,native);
    // These queries only inspect the restored model. In particular a chat
    // refresh must not detach and diff the entire durable history again.
    // recentlyDeleted is excluded: reading it expires persisted recovery rows.
    if(source==='conversation' || source==='conversationDraft' || source==='inbox' || source==='recipients' || source==='syncState')return sources[source](args,store,storage,native);
    if(source==='advanceReplies' && idleReplyTick(Number(args[0])))return changed();
    const records=editRecords(source,args);
    // These sources leave reply scheduling untouched. Future sources retain the
    // conservative full copy until their pending-state behavior is established.
    const keepsPending=['markRead','setConversationUnread','muteConversation','saveDraft','react','createLocalContact','recentlyDeleted','purgeConversations','deleteMessages','recoverConversations'].includes(source)
      || (source==='blockConversation' && !args[1])
      || ((source==='blockConversation' || source==='deleteConversation') && !pending.has(args[0] as string));
    // Unblocking and removing an absent schedule leave the map untouched. An
    // actual removal still needs the full copy to restore its insertion order.
    // A send only sets its own entry. Map.set preserves an existing entry's
    // position, so rollback needs one value rather than a copy of every reply.
    const sentId=source==='sendMessage'?String(args[0]):undefined;
    const previousActivity=sentId===undefined?undefined:pending.get(sentId);
    const previousPending=keepsPending||sentId!==undefined?undefined:new Map(pending),previousTicks=ticks,previousRevision=revision,previousPendingTime=nextPendingTime;
    const value=await sources[source](args,store,storage,native);
    // Reply ticks change durable records only when a receipt or reply advances
    // revision. Keep their clock update, but avoid copying an unchanged history.
    // Other sources can expire recovery rows without changing revision.
    if(source==='advanceReplies' && revision===previousRevision)return value;
    try{await client.edit(records());}catch(error){
      // Undo before restore prunes schedules for absent/deleted/blocked threads.
      if(sentId!==undefined){if(previousActivity)pending.set(sentId,previousActivity);else pending.delete(sentId);}
      restore(client.initial());if(previousPending){pending.clear();for(const [id,activity] of previousPending)if(!deleted.has(id)&&!blocked.has(id)&&threads.has(id))pending.set(id,activity);}ticks=previousTicks;
      nextPendingTime=previousPendingTime;
      // A failed save must not leave an invalid model that every later read
      // tries to save again. Report the original error even if disk is full.
      try{await client.failed(error);}catch{/* The runner still receives the save failure. */}
      throw error;
    }
    return value;
  });
  };
  // Loading an optional artifact is network work too: finish it before taking
  // the local edit queue, so simultaneous first reads each retain a live ticket.
  return core===undefined?loadBrowserDevice().then(run):run();
};
