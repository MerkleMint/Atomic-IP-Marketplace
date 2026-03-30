import { ThemeToggle } from './components/ThemeToggle';
import { 
  ShieldCheck, 
  RotateCw, 
  Search, 
  Zap, 
  Lock, 
  Globe,
  Plus
} from 'lucide-react';

const IP_ASSETS = [
  {
    id: 1,
    title: "Eco-Friendly Battery Tech",
    type: "Patent",
    price: "1,200 XLM",
    owner: "GreenLabs.sol",
    description: "A novel solid-state battery architecture for high-efficiency energy storage."
  },
  {
    id: 2,
    title: "Quantum Encryption Algorithm",
    type: "Software Copyright",
    price: "5,000 XLM",
    owner: "CryptoSec.sol",
    description: "Post-quantum secure key exchange protocol for financial applications."
  },
  {
    id: 3,
    title: "Sustainable Water Filter",
    type: "Utility Model",
    price: "850 XLM",
    owner: "BluePlanet.sol",
    description: "Low-cost graphene-based filtration system for decentralized water purifying."
  }
];

function App() {
  return (
    <div className="min-h-screen bg-background text-foreground transition-colors duration-300">
      <ThemeToggle />

      {/* Header */}
      <header className="border-b border-primary/10 sticky top-0 bg-background/80 backdrop-blur-md z-40">
        <div className="max-w-7xl mx-auto px-4 h-16 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="p-2 bg-primary rounded-lg text-primary-foreground">
              <ShieldCheck className="w-6 h-6" />
            </div>
            <span className="font-bold text-xl tracking-tight">Atomic IP</span>
          </div>
          
          <nav className="hidden md:flex items-center gap-6">
            <a href="#" className="text-sm font-medium hover:text-primary transition-colors">Marketplace</a>
            <a href="#" className="text-sm font-medium hover:text-primary transition-colors">Registry</a>
            <a href="#" className="text-sm font-medium hover:text-primary transition-colors">About</a>
          </nav>

          <div className="flex items-center gap-4">
            <div className="relative group">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground group-focus-within:text-primary transition-colors" />
              <input 
                type="text" 
                placeholder="Search IP Assets..."
                className="pl-10 pr-4 py-1.5 rounded-full bg-secondary border-none focus:ring-2 focus:ring-primary/50 transition-all w-48 focus:w-64"
              />
            </div>
            <button className="bg-primary text-primary-foreground px-4 py-2 rounded-full text-sm font-medium hover:opacity-90 transition-opacity flex items-center gap-2">
              <Plus className="w-4 h-4" /> Mint IP
            </button>
          </div>
        </div>
      </header>

      {/* Hero Section */}
      <main className="max-w-7xl mx-auto px-4 py-12">
        <div className="relative rounded-3xl overflow-hidden bg-primary/5 border border-primary/10 p-12 md:p-24 text-center mb-16 group">
          <div className="absolute top-0 left-0 w-full h-full overflow-hidden z-0 pointer-events-none">
             <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/20 rounded-full blur-[100px] animate-pulse"></div>
             <div className="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-purple-500/10 rounded-full blur-[100px] animate-pulse delay-700"></div>
          </div>
          
          <div className="relative z-10">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-primary/10 text-primary text-xs font-bold mb-6 tracking-wide">
              <Zap className="w-3 h-3 fill-primary" /> POWERED BY SOROBAN
            </div>
            <h1 className="text-5xl md:text-7xl mb-6 bg-gradient-to-r from-foreground to-foreground/60 bg-clip-text text-transparent">
              The Future of <br /> IP Ownership
            </h1>
            <p className="text-lg text-muted-foreground max-w-2xl mx-auto mb-10">
              Register, manage, and swap Intellectual Property assets with atomic settlement. Finalize ownership transfers instantly with Zero-Knowledge verification.
            </p>
            <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
              <button className="px-8 py-3 bg-primary text-primary-foreground rounded-xl font-bold shadow-lg shadow-primary/20 hover:scale-105 transition-transform w-full sm:w-auto">
                Explore Marketplace
              </button>
              <button className="px-8 py-3 bg-secondary text-secondary-foreground border border-border rounded-xl font-bold hover:bg-border/50 transition-colors w-full sm:w-auto">
                How it works
              </button>
            </div>
          </div>
        </div>

        {/* Features Split */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-8 mb-24">
          <div className="p-8 rounded-2xl bg-card border border-primary/10 hover:shadow-xl hover:-translate-y-1 transition-all">
            <div className="w-12 h-12 bg-blue-500/10 rounded-xl flex items-center justify-center mb-6">
              <ShieldCheck className="w-6 h-6 text-blue-500" />
            </div>
            <h3 className="text-xl font-bold mb-2">Immutable Registry</h3>
            <p className="text-muted-foreground text-sm leading-relaxed">
              Every IP asset is securely recorded on the Stellar ledger, ensuring tamper-proof proof of existence.
            </p>
          </div>
          <div className="p-8 rounded-2xl bg-card border border-border hover:shadow-xl hover:-translate-y-1 transition-all">
            <div className="w-12 h-12 bg-green-500/10 rounded-xl flex items-center justify-center mb-6">
              <RotateCw className="w-6 h-6 text-green-500" />
            </div>
            <h3 className="text-xl font-bold mb-2">Atomic Swaps</h3>
            <p className="text-muted-foreground text-sm leading-relaxed">
              Trustless swaps ensure that payment and IP transfer happen simultaneously or not at all.
            </p>
          </div>
          <div className="p-8 rounded-2xl bg-card border border-primary/10 hover:shadow-xl hover:-translate-y-1 transition-all">
            <div className="w-12 h-12 bg-purple-500/10 rounded-xl flex items-center justify-center mb-6">
              <Lock className="w-6 h-6 text-purple-500" />
            </div>
            <h3 className="text-xl font-bold mb-2">ZK-Verification</h3>
            <p className="text-muted-foreground text-sm leading-relaxed">
              Verify legal compliance and ownership without revealing sensitive patent details publicly.
            </p>
          </div>
        </div>

        {/* Featured Assets */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <h2 className="text-3xl font-bold">Featured IP Assets</h2>
            <button className="text-primary text-sm font-bold hover:underline flex items-center gap-1">
              View All <Globe className="w-4 h-4" />
            </button>
          </div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
            {IP_ASSETS.map((asset) => (
              <div key={asset.id} className="group bg-card border border-primary/10 rounded-2xl overflow-hidden hover:border-primary/50 transition-colors">
                <div className="aspect-video bg-muted relative flex items-center justify-center px-12 group-hover:bg-primary/5 transition-colors">
                  <div className="w-24 h-24 bg-card rounded-2xl shadow-sm flex items-center justify-center border border-primary/10 group-hover:scale-110 transition-transform">
                    <ShieldCheck className="w-12 h-12 text-primary opacity-20 group-hover:opacity-100 transition-opacity" />
                  </div>
                </div>
                <div className="p-6">
                  <div className="flex items-center justify-between mb-3 text-xs font-bold uppercase tracking-wider text-muted-foreground">
                    <span>{asset.type}</span>
                    <span className="text-primary">{asset.owner}</span>
                  </div>
                  <h4 className="text-lg font-bold mb-2 group-hover:text-primary transition-colors underline-offset-4 decoration-primary/50 group-hover:underline">{asset.title}</h4>
                  <p className="text-muted-foreground text-sm mb-6 line-clamp-2">
                    {asset.description}
                  </p>
                  <div className="flex items-center justify-between">
                    <div>
                      <div className="text-[10px] text-muted-foreground uppercase font-bold tracking-widest">ASKING PRICE</div>
                      <div className="text-lg font-black">{asset.price}</div>
                    </div>
                    <button className="bg-foreground text-background px-4 py-2 rounded-lg text-sm font-bold hover:opacity-80 transition-opacity">
                      Quick Buy
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </section>
      </main>

      {/* Footer */}
      <footer className="border-t border-primary/10 mt-24 py-12 bg-secondary/30">
        <div className="max-w-7xl mx-auto px-4 flex flex-col md:flex-row items-center justify-between gap-8">
          <div className="flex items-center gap-2 opacity-50 grayscale hover:opacity-100 hover:grayscale-0 transition-all">
            <div className="p-1 px-2 border border-foreground rounded text-xs font-black">Soroban</div>
            <div className="text-xs font-bold tracking-widest uppercase">Smart Contracts</div>
          </div>
          <div className="text-xs text-muted-foreground font-medium">
            &copy; 2026 Atomic IP Marketplace. Securely trading innovation.
          </div>
          <div className="flex items-center gap-6">
            <a href="#" className="text-xs text-muted-foreground hover:text-foreground">Terms</a>
            <a href="#" className="text-xs text-muted-foreground hover:text-foreground">Privacy</a>
            <a href="#" className="text-xs text-muted-foreground hover:text-foreground">GitHub</a>
          </div>
        </div>
      </footer>
    </div>
  );
}

export default App;
