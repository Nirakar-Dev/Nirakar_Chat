import { useState, useEffect } from 'react';
import { TitleBar } from './components/TitleBar';
import { Settings, Send, Search, Copy, Check, Plus, Trash2, Save, Pencil } from 'lucide-react';
import { Avatar, AvatarFallback, AvatarImage } from './components/ui/avatar';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { ScrollArea } from './components/ui/scroll-area';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from './components/ui/dialog';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export interface Contact {
  id: string;
  peer_id: string;
  name: string;
  online: boolean;
}

interface Identity {
  nirakar_id: string;
  public_key: string;
  name: string;
}

interface NetworkStatus {
  status: string;
  peer_id: string;
  message: string;
}

interface ChatMessage {
  id: number;
  peer_id: string;
  is_outgoing: boolean;
  content: string;
  timestamp: string;
}

export default function App() {
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [activeContact, setActiveContact] = useState<Contact | null>(null);
  const [identity, setIdentity] = useState<Identity | null>(null);
  const [copied, setCopied] = useState(false);
  const [networkStatus, setNetworkStatus] = useState<NetworkStatus | null>(null);
  const [newContactId, setNewContactId] = useState('');
  const [isAddContactOpen, setIsAddContactOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [editName, setEditName] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [editingContactId, setEditingContactId] = useState<string | null>(null);
  const [editContactName, setEditContactName] = useState('');

  const loadContacts = async () => {
    try {
      const fetchedContacts = await invoke<Contact[]>('fetch_contacts');
      setContacts(fetchedContacts);
      if (fetchedContacts.length > 0 && !activeContact) {
        setActiveContact(fetchedContacts[0]);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const fetchMessages = async (peerId: string) => {
    try {
      const msgs = await invoke<ChatMessage[]>('fetch_messages', { peerId });
      setMessages(msgs);
    } catch (e) {
      console.error(e);
    }
  };

  const handleSendMessage = async () => {
    if (!inputMessage.trim() || !activeContact) return;
    try {
      await invoke('send_message', { peerId: activeContact.id, content: inputMessage });
      setInputMessage('');
      fetchMessages(activeContact.id);
    } catch (e) {
      console.error(e);
    }
  };

  const handleAddContact = async () => {
    try {
      await invoke('add_contact', { nirakarId: newContactId, multiaddrStr: null });
      setIsAddContactOpen(false);
      setNewContactId('');
    } catch (e) {
      console.error(e);
    }
  };

  const handleSaveProfile = async () => {
    if (!editName.trim()) return;
    try {
      const updated = await invoke<Identity>('update_name', { newName: editName });
      setIdentity(updated);
      setSettingsOpen(false);
    } catch (e) {
      console.error(e);
    }
  };

  const handleDeleteContact = async (contactId: string) => {
    try {
      await invoke('delete_contact', { contactId });
      if (activeContact?.id === contactId) {
        setActiveContact(null);
        setMessages([]);
      }
      loadContacts();
    } catch (e) {
      console.error(e);
    }
  };

  const handleRenameContact = async (contactId: string) => {
    if (!editContactName.trim()) return;
    try {
      await invoke('rename_contact', { contactId, newName: editContactName });
      setEditingContactId(null);
      setEditContactName('');
      loadContacts();
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    loadContacts();

    invoke<Identity>('get_identity')
      .then((id) => {
        setIdentity(id);
        invoke('publish_identity', { nirakarId: id.nirakar_id }).catch(console.error);
      })
      .catch(console.error);

    let unlistenNetwork: UnlistenFn | undefined;
    
    listen<NetworkStatus>('network_status', (event) => {
      setNetworkStatus(event.payload);
      if (event.payload.status === 'Found Peer') {
        loadContacts();
      }
    }).then(fn => { unlistenNetwork = fn; });
    
    return () => {
      if (unlistenNetwork) unlistenNetwork();
    };
  }, []);

  useEffect(() => {
    if (activeContact) {
      fetchMessages(activeContact.id);
    }
  }, [activeContact]);

  useEffect(() => {
    let unlistenChat: UnlistenFn | undefined;
    listen<string>('chat_update', (event) => {
      if (activeContact && event.payload === activeContact.id) {
        fetchMessages(activeContact.id);
      }
    }).then(fn => { unlistenChat = fn; });

    let unlistenFileDrop: UnlistenFn | undefined;
    listen<{ paths: string[] }>('tauri://file-drop', (event) => {
      if (activeContact && event.payload.paths.length > 0) {
        for (const path of event.payload.paths) {
          invoke('send_file', { peerId: activeContact.id, filePath: path }).catch(console.error);
        }
      }
    }).then(fn => { unlistenFileDrop = fn; });

    return () => {
      if (unlistenChat) unlistenChat();
      if (unlistenFileDrop) unlistenFileDrop();
    };
  }, [activeContact]);

  const handleCopyId = async () => {
    if (identity) {
      try {
        await navigator.clipboard.writeText(identity.nirakar_id);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch (err) {
        console.error('Failed to copy', err);
      }
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-zinc-950 text-zinc-100 overflow-hidden font-sans">
      <TitleBar />
      
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <div className="w-72 border-r border-white/5 bg-zinc-900/40 flex flex-col backdrop-blur-md">
          {/* Profile Area */}
          <div className="p-4 border-b border-white/5 flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Avatar className="h-10 w-10 border border-white/10 ring-2 ring-purple-500/20">
                <AvatarImage src="/avatar.png" alt="Profile" />
                <AvatarFallback className="bg-zinc-800 text-purple-400">
                  {identity ? identity.name.substring(0, 2).toUpperCase() : 'NK'}
                </AvatarFallback>
              </Avatar>
              <div>
                <h3 className="font-medium text-sm text-zinc-200">{identity ? identity.name : 'My Profile'}</h3>
                <p className="text-xs text-purple-400/80 font-mono nirakar-text-glow">
                  ID: {identity ? identity.nirakar_id : 'Loading...'}
                </p>
              </div>
            </div>

            <Dialog open={settingsOpen} onOpenChange={(open) => {
              setSettingsOpen(open);
              if (open && identity) setEditName(identity.name);
            }}>
              <DialogTrigger asChild>
                <Button variant="ghost" size="icon" className="text-zinc-400 hover:text-purple-400 hover:bg-purple-500/10 transition-colors">
                  <Settings className="w-5 h-5" />
                </Button>
              </DialogTrigger>
              <DialogContent className="sm:max-w-[425px] bg-zinc-950 border-white/10 text-zinc-100">
                <DialogHeader>
                  <DialogTitle>Settings</DialogTitle>
                  <DialogDescription className="text-zinc-400">
                    Manage your Nirakar profile and preferences.
                  </DialogDescription>
                </DialogHeader>
                <div className="grid gap-4 py-4">
                  <div className="flex items-center gap-4">
                    <Avatar className="h-16 w-16 border border-white/10 ring-2 ring-purple-500/20">
                      <AvatarImage src="/avatar.png" alt="Profile" />
                      <AvatarFallback className="bg-zinc-800 text-purple-400 text-lg">
                        {identity ? identity.name.substring(0, 2).toUpperCase() : 'NK'}
                      </AvatarFallback>
                    </Avatar>
                    <div className="flex-1">
                      <p className="text-xs text-zinc-500 mb-1">Display Name</p>
                      <Input 
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        className="bg-zinc-900 border-white/10 focus-visible:ring-purple-500" 
                      />
                    </div>
                  </div>
                  <div className="grid grid-cols-4 items-center gap-4">
                    <label className="text-right text-sm font-medium text-zinc-300">Nirakar ID</label>
                    <div className="col-span-3 flex items-center gap-2">
                      <code className="bg-zinc-900 px-3 py-1.5 rounded-md text-sm text-purple-400 border border-white/5 flex-1 font-mono">
                        {identity?.nirakar_id || 'Loading...'}
                      </code>
                      <Button variant="outline" size="icon" onClick={handleCopyId} className="border-white/10 hover:bg-white/5 hover:text-white shrink-0">
                        {copied ? <Check className="w-4 h-4 text-green-500" /> : <Copy className="w-4 h-4" />}
                      </Button>
                    </div>
                  </div>
                  <div className="grid grid-cols-4 items-center gap-4">
                    <label className="text-right text-sm font-medium text-zinc-300">Theme</label>
                    <div className="col-span-3 text-sm text-purple-400">Ethereal Dark (Default)</div>
                  </div>
                </div>
                <div className="flex justify-end">
                  <Button onClick={handleSaveProfile} className="bg-purple-600 hover:bg-purple-500 text-white gap-2">
                    <Save className="w-4 h-4" /> Save Changes
                  </Button>
                </div>
              </DialogContent>
            </Dialog>
          </div>

          {/* Search Contacts and Add Contact */}
          <div className="p-4 space-y-4">
            <div className="flex items-center justify-between">
              <h2 className="font-semibold tracking-wide text-zinc-100 flex items-center gap-2">
                <span className="w-1.5 h-1.5 rounded-full bg-purple-500 nirakar-glow"></span>
                Contacts
              </h2>
              <Dialog open={isAddContactOpen} onOpenChange={setIsAddContactOpen}>
                <DialogTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-8 w-8 text-zinc-400 hover:text-white hover:bg-white/10 rounded-full transition-all duration-300">
                    <Plus className="w-4 h-4" />
                  </Button>
                </DialogTrigger>
                <DialogContent className="sm:max-w-md bg-zinc-950 border-zinc-800 text-zinc-100 p-0 overflow-hidden shadow-2xl shadow-purple-900/20">
                  <div className="p-6">
                    <DialogHeader className="mb-6">
                      <DialogTitle className="text-xl font-medium flex items-center gap-2">
                        Add Contact
                      </DialogTitle>
                    </DialogHeader>
                    <div className="space-y-4">
                      <div>
                        <label className="text-xs font-medium text-zinc-400 uppercase tracking-wider mb-2 block">Nirakar ID</label>
                        <div className="relative">
                          <input 
                            type="text" 
                            placeholder="NK-XXXXX"
                            value={newContactId}
                            onChange={(e) => setNewContactId(e.target.value)}
                            className="w-full bg-zinc-900/50 border border-zinc-800 rounded-lg p-3 text-zinc-100 font-mono text-sm focus:outline-none focus:border-purple-500/50 focus:ring-1 focus:ring-purple-500/50 transition-all"
                          />
                        </div>
                      </div>
                      <Button onClick={handleAddContact} className="w-full bg-purple-600 hover:bg-purple-500 text-white shadow-lg shadow-purple-900/50 transition-all">
                        Connect via DHT
                      </Button>
                    </div>
                  </div>
                </DialogContent>
              </Dialog>
            </div>
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
              <Input 
                placeholder="Search entities..." 
                className="pl-9 bg-zinc-950/50 border-white/5 focus-visible:ring-purple-500/50 text-sm h-9"
              />
            </div>
          </div>

          {/* Contacts List */}
          <ScrollArea className="flex-1">
            <div className="p-2 space-y-1">
              {contacts.length === 0 && (
                <p className="text-center text-xs text-zinc-500 py-4">No contacts found</p>
              )}
              {contacts.map((contact) => (
                <div
                  key={contact.id}
                  className={`group w-full flex items-center gap-3 p-3 rounded-lg transition-all cursor-pointer ${
                    activeContact?.id === contact.id 
                      ? 'bg-purple-500/10 border border-purple-500/20' 
                      : 'hover:bg-white/5 border border-transparent'
                  }`}
                  onClick={() => {
                    setActiveContact(contact);
                    setEditingContactId(null);
                  }}
                >
                  <div className="relative">
                    <Avatar className="h-10 w-10 bg-zinc-800">
                      <AvatarImage src="/avatar.png" alt={contact.name} />
                      <AvatarFallback className="text-zinc-400">{contact.name[0]}</AvatarFallback>
                    </Avatar>
                    <span 
                      className={`absolute bottom-0 right-0 w-3 h-3 rounded-full border-2 border-zinc-900 ${
                        contact.online ? 'bg-green-500 nirakar-glow' : 'bg-zinc-600'
                      }`} 
                    />
                  </div>
                  {editingContactId === contact.id ? (
                    <div className="flex-1 flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
                      <Input 
                        value={editContactName}
                        onChange={(e) => setEditContactName(e.target.value)}
                        onKeyDown={(e) => { if (e.key === 'Enter') handleRenameContact(contact.id); }}
                        className="h-7 text-xs bg-zinc-900 border-white/10 focus-visible:ring-purple-500"
                        autoFocus
                      />
                      <Button size="icon" variant="ghost" className="h-7 w-7 text-green-400 hover:text-green-300" onClick={() => handleRenameContact(contact.id)}>
                        <Check className="w-3 h-3" />
                      </Button>
                    </div>
                  ) : (
                    <div className="flex-1 text-left min-w-0">
                      <h4 className="text-sm font-medium text-zinc-200 truncate">{contact.name}</h4>
                      <p className="text-xs text-zinc-500 font-mono truncate">{contact.id}</p>
                    </div>
                  )}
                  <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity" onClick={(e) => e.stopPropagation()}>
                    <Button size="icon" variant="ghost" className="h-7 w-7 text-zinc-500 hover:text-purple-400" onClick={() => {
                      setEditingContactId(contact.id);
                      setEditContactName(contact.name);
                    }}>
                      <Pencil className="w-3 h-3" />
                    </Button>
                    <Button size="icon" variant="ghost" className="h-7 w-7 text-zinc-500 hover:text-red-400" onClick={() => handleDeleteContact(contact.id)}>
                      <Trash2 className="w-3 h-3" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>
        </div>

        {/* Main Chat Area */}
        <div className="flex-1 flex flex-col relative bg-zinc-950/80">
          {/* Subtle Background Glow */}
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[500px] h-[500px] bg-purple-900/10 blur-[100px] rounded-full pointer-events-none" />

          {activeContact ? (
            <>
              {/* Chat Header */}
              <div className="h-16 border-b border-white/5 flex items-center justify-between px-6 bg-zinc-900/20 backdrop-blur-sm z-10">
                <div className="flex items-center gap-3">
                  <div className="relative">
                    <Avatar className="h-8 w-8 bg-zinc-800">
                      <AvatarImage src="/avatar.png" alt={activeContact.name} />
                      <AvatarFallback className="text-zinc-400 text-xs">{activeContact.name[0]}</AvatarFallback>
                    </Avatar>
                    <span 
                      className={`absolute bottom-0 right-0 w-2.5 h-2.5 rounded-full border-2 border-zinc-900 ${
                        activeContact.online ? 'bg-green-500' : 'bg-zinc-600'
                      }`} 
                    />
                  </div>
                  <div>
                    <h3 className="font-medium text-sm text-zinc-100">{activeContact.name}</h3>
                    <p className="text-xs text-purple-400/80 font-mono nirakar-text-glow">{activeContact.id}</p>
                  </div>
                </div>
            
            <div className="text-right">
              <div className="flex items-center gap-2 justify-end">
                <span className={`w-2 h-2 rounded-full ${
                  networkStatus?.status === 'Connected' ? 'bg-green-500 nirakar-glow' :
                  networkStatus?.status === 'Connecting' ? 'bg-yellow-500 animate-pulse' :
                  'bg-red-500'
                }`} />
                <span className="text-xs font-medium text-zinc-300">
                  {networkStatus?.status || 'Offline'}
                </span>
              </div>
              <p className="text-[10px] text-zinc-500 font-mono mt-0.5 truncate max-w-[200px]">
                {networkStatus?.message || 'Waiting for network...'}
              </p>
            </div>
          </div>

          {/* Messages */}
          <ScrollArea className="flex-1 p-6 z-10">
            <div className="space-y-6">
              <div className="flex justify-center">
                <span className="text-xs text-zinc-600 bg-zinc-900/50 px-3 py-1 rounded-full border border-white/5">
                  End-to-end encrypted connection established
                </span>
              </div>
              
              {messages.map((msg) => (
                <div key={msg.id} className={`flex gap-4 ${msg.is_outgoing ? 'flex-row-reverse' : ''}`}>
                  {!msg.is_outgoing && (
                    <Avatar className="h-8 w-8 mt-1">
                      <AvatarImage src="/avatar.png" alt={activeContact.name} />
                      <AvatarFallback className="bg-zinc-800 text-zinc-400">{activeContact.name[0]}</AvatarFallback>
                    </Avatar>
                  )}
                  <div className={`${
                    msg.is_outgoing 
                      ? 'bg-purple-600/20 border-purple-500/30 rounded-tr-none' 
                      : 'bg-zinc-900/80 border-white/10 rounded-tl-none'
                    } border rounded-2xl p-3 max-w-[70%]`}
                  >
                    <p className={`text-sm ${msg.is_outgoing ? 'text-zinc-100' : 'text-zinc-200'}`}>
                      {msg.content}
                    </p>
                    <span className={`text-[10px] mt-1 block ${msg.is_outgoing ? 'text-purple-300/50 text-right' : 'text-zinc-500'}`}>
                      {new Date(msg.timestamp + 'Z').toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </ScrollArea>

          {/* Input Area */}
          <div className="p-4 bg-zinc-900/30 border-t border-white/5 backdrop-blur-sm z-10">
            <div className="flex items-center gap-2 max-w-4xl mx-auto">
              <Input 
                placeholder="Send a transmission..." 
                value={inputMessage}
                onChange={(e) => setInputMessage(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleSendMessage(); }}
                className="flex-1 bg-zinc-950/50 border-white/10 focus-visible:ring-purple-500/50 h-12 rounded-xl text-zinc-200 placeholder:text-zinc-600"
              />
              <Button 
                onClick={handleSendMessage}
                size="icon" 
                className="h-12 w-12 rounded-xl bg-purple-600 hover:bg-purple-500 text-white shadow-[0_0_15px_rgba(168,85,247,0.4)] transition-all"
              >
                <Send className="w-5 h-5" />
              </Button>
            </div>
          </div>
          </>
          ) : (
            <div className="flex-1 flex items-center justify-center z-10">
              <p className="text-zinc-500 text-sm">Select a contact to start transmitting</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
