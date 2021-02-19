#include <cstdio>
#include <queue>
#include <cstring>
#include <set>
#include <cassert>
#include <filesystem>
#include <fstream>
#include <inttypes.h>
#include <iostream>
#include <map>
#include <ostream>
#include <sstream>
#include <stdio.h>
#include <string>
#include <sys/uio.h>
#include <vector>
#include <sys/mman.h>

#define MAGIC 0x12345678991301
#define MAGIC_RUST 0x12345678991101

// #define DEBUG

using namespace std;

const double MiB = (double)1024*1024;
const int min_printable_size = MiB/ 2;
const bool COMPUTE_TRACES = false;


string tolower(string in) {
  for (auto &c: in) c = tolower(c);

  return in;
}

vector<string> split_string(string s) {
  vector<string> res;
  string temp = "";
  for(size_t i=0;i<s.length();++i){
    if (s[i] == ' '){
      res.push_back(temp);
      temp = "";
    } else {
      temp.push_back(s[i]);
    }
  }
  res.push_back(temp);
  return res;
}

vector<string> split_string2(string s) {
  // split by 2 spaces
  vector<string> res;
  string temp = "";
  int cnt = 0;
  for(size_t i=0;i<s.length();++i){
    if (s[i] == ' '){
      cnt += 1;
      if (cnt <= 1) {
         temp.push_back(s[i]);
      } else {
          res.push_back(temp);
          temp = "";
      }
    } else {
      cnt = 0;
      temp.push_back(s[i]);
    }
  }
  res.push_back(temp);
  return res;
}

struct Entry {
  string file;
  uint64_t from, to;
};

struct Mmap {
  uint64_t func;
  uint64_t to;
};

struct Counter {
  uint64_t sum;
  uint64_t cnt;
  vector <uint64_t> samples;
};

map<uint64_t, Mmap> read_mmap() {
  map<uint64_t, Mmap> res;

  for (const auto & entry : std::filesystem::directory_iterator("/tmp/dump/logs")) {
    ifstream in(entry.path(), ios::in);

    const int SIZE=10000;
    char str[SIZE];
    string file = "";
    map<uint64_t, Mmap> tmp_res;
    while(in.getline(str,SIZE)){
      if (string(str).rfind("mmap ", 0) == 0) {
        istringstream is(str);
        string nop;
        uint64_t addr, len, func, flags;

        is>>nop>>hex>>addr>>dec>>len>>hex>>func>>flags;
#ifdef DEBUG
        cout<<hex<<addr<<" "<<hex<<len<<" "<<dec<<func<<endl;
#endif

        tmp_res[addr] = {func, addr+len};
      }
      if (string(str).rfind("munmap ", 0) == 0) {
        istringstream is(str);
        string nop;
        uint64_t addr;
        is>>nop>>hex>>addr;
        tmp_res.erase(addr);

      }
    }
    for (auto elem : tmp_res) {
      res[elem.first] = elem.second;
    }
  }
#ifdef DEBUG
  cout<<"mmap = " << res.size()<<endl;
#endif

  return res;

}

vector<Entry> read_symbols2(int pid) {
  vector<Entry> res;
  ostringstream symbols_file;
  symbols_file << "/tmp/dump/symbols/" << pid << ".m";

  ifstream in(symbols_file.str().c_str(), ios::in);

  const int SIZE=100000;
  char str[SIZE];

  string file = "";
  while(in.getline(str,SIZE)){
    if (str[0] != '[') continue;

    auto tmp = split_string(str);
    auto tmp2 = split_string2(str);
    if (tmp.size() < 7) continue;
    uint64_t addr = 0;
    sscanf(tmp[2].c_str(), "%p", (void**)&addr);

    if (tmp2.size() >= 3) {
      file = tmp2[1];
    } else {
      file = tmp[3];
    }
    res.push_back({file, addr, addr});
  }
  // last elem will be skipped
  return res;
}

vector<Entry> read_symbols(int pid) {
  vector<Entry> res;
  ostringstream symbols_file;
  symbols_file << "/tmp/dump/symbols/" << pid;

  ifstream in(symbols_file.str().c_str(), ios::in);

  const int SIZE=100000;
  char str[SIZE];

  string file = "";
  while(in.getline(str,SIZE)){
    string s = str;
    if (s.rfind("Partial symtab for source file", 0) == 0) {
      auto tmp = split_string(s);
      file = tmp[5];
#ifdef DEBUG
      cout<<"file = "<<file<<endl;
#endif
    }
    if (s.rfind("  Symbols cover text addresses ", 0) == 0) {
      auto tmp = split_string(s);
      uint64_t a1, a2;
      sscanf( tmp[6].c_str(), "%" PRIx64 "-%" PRIx64, &a1, &a2);
#ifdef DEBUG
      cout<<file<<" "<<tmp[6]<<endl;
      printf("MAPPING 0x%" PRIx64 "-0x%" PRIx64 "\n", a1, a2);
#endif
      Entry e = {file, a1, a2};
      res.push_back(e);
    }
  }
  return res;
}

const int PAGE_SIZE = 4096;
const int PAGEMAP_ENTRY = 8;

const int ALLOC_LIMIT = 100;


bool is_page_present(FILE *f, uint64_t addr) {
  fseek(f, addr/PAGE_SIZE*PAGEMAP_ENTRY, SEEK_SET);
  uint64_t mask=0;
  size_t n = fread(&mask,1,sizeof(mask),f);
  if (n == 0) return false;

  return (mask >> 63) & 1;
}

uint64_t calc_size(FILE *f, uint64_t from, uint64_t size) {
  uint64_t res = 0;
  uint64_t to = from + size;
  while (from < to) {
    bool present = is_page_present(f, from);
    uint64_t next = (from + PAGE_SIZE) / PAGE_SIZE * PAGE_SIZE;

    if (present) {
      res += min(to - from, next - from);
    }


    from = next;
  }
  return res;
}


struct Smap {
  uint64_t from, to;
  string mapped_file;

  bool is_stack = false;
  uint64_t offset;
};

bool hasEnding (std::string const &fullString, std::string const &ending) {
    if (fullString.length() >= ending.length()) {
        return 0 == fullString.compare (fullString.length() - ending.length(), ending.length(), ending);
    } else {
        return false;
    }
}

vector<Smap> read_smaps(int pid) {
  const int SIZE = 1024;
  char str[SIZE];
  ostringstream maps_file_name, pagemap_file_name;
  maps_file_name << "/proc/" << pid << "/smaps";
  ifstream smaps_in(maps_file_name.str().c_str(), ios::in);
  vector<Smap> result;

  while (smaps_in.getline(str,SIZE)) {
    uint64_t a1 = 0, a2 = 0, offset = 0;
    char flags[256];

    // 7f34d3290000-7f34e3a00000 r--p 00000000 0
    if (sscanf( str, "%" PRIx64 "-%" PRIx64 " %s %" PRIX64, &a1, &a2, flags, &offset) >= 2) {
      sscanf( str, "%" PRIx64 "-%" PRIx64, &a1, &a2);
      string mapped_file;
      if (strlen(str) >= 73) {
        mapped_file = string(&str[73]);
      }
      result.push_back((Smap){a1, a2, mapped_file, false, offset});
    }
    if (result.size() > 0 && strncmp(str, "VmFlags:", strlen("VmFlags:")) == 0) {
      stringstream ss(str);
      string tmp;
      while (ss >> tmp) if (tmp == "gd") result.back().is_stack = true;

    }

  }
  cout<<"read_maps "<<result.size()<<endl;
  return result;
}

uint64_t get_func_bin_ptr(uint64_t ptr, map<uint64_t, Smap> &smaps_cache) {
  auto x = smaps_cache.upper_bound(ptr);
  if (x == smaps_cache.begin()) {
    return ptr;
  }
  x--;

  if (!(x->second.from <= ptr && ptr < x->second.to)) return 0;

  return ptr - x->second.from + x->second.offset;
}


struct BlockInfo {
  uint64_t size;
  uint64_t func;
  bool is_c;
  bool is_freed;
  uint64_t ref;
  vector<uint64_t> prev;
};

struct ResultEntry {
  int depth;
  uint64_t ptr, size;
  string path;
  uint64_t func;
};


int main(int argc, char **argv) {
  if (argc < 2) {
    cout << "./dump <PID> [<TID>]" << endl;
    return 1;
  }
  auto mmap = read_mmap();
  int pid = atoi(argv[1]);
  uint64_t target_tid = 0;
  if (argc >= 3) {
    target_tid = atoi(argv[2]);
  }
  auto symbols = read_symbols(pid);
  auto symbols2 = read_symbols2(pid);

  map<uint64_t, Entry> symbols2_cache;
  for(auto elem:symbols2) symbols2_cache[elem.from] = elem;

  auto find_file2 = [&](uint64_t ptr) {
    if (ptr == 1) return string("<UNKNOWN1>");
    if (ptr == 2) return string("<UNKNOWN2>");

    string file2 = "<UNKNOWN>";

    auto x = symbols2_cache.upper_bound(ptr);
    if (x == symbols2_cache.begin()) return file2;
    x--;

    return x->second.file;
  };

  cout << "PID: " << pid << " target_tid " << target_tid << endl;
  stringstream pagemap_file_name;
  pagemap_file_name << "/proc/" << pid << "/pagemap";



  ifstream pmap(pagemap_file_name.str().c_str(), ios::in | ios::binary);
  FILE *f = fopen(pagemap_file_name.str().c_str(), "rb");

  uint64_t pages = 0;
  uint64_t active_pages = 0, tracked_memory = 0, not_mmaped_file_size=0, mmaped_file_size = 0, c_active_pages = 0; //, non_mmaped_file_size = 0;
  uint64_t allocator_headers = 0;

  map<uint64_t, Counter> mmap_func2size, func2size_c, func2size_c_small, func2size_c_dealloc, tid2size_c, mmap_func2size2, func2size_rust, func2size_rust_small, func2size_rust_dealloc;
  map<string, Counter> file2size_c, mmap_file2size, mmap_file2size2, file2size_rust;

  uint64_t overlap_c_mmap = 0;
  uint64_t unknown_mem = 0;
  uint64_t unknown_mem_possibly_c = 0;
  uint64_t wasmer_size = 0, rocksdb_size = 0, libbacktrace_size = 0, near_size = 0;
  uint64_t wasmer_cnt = 0, rocksdb_cnt = 0, libbacktrace_cnt = 0, near_cnt = 0;

  auto smaps = read_smaps(pid);
  map<uint64_t, Smap> smaps_cache;
  for(auto elem:smaps) smaps_cache[elem.from] = elem;

  uint64_t dealloc_c = 0, dealloc_rust = 0, dealloc_func = 0;


  map <uint64_t, BlockInfo> ptr2bi;

  for (auto smap : smaps) {
    uint64_t tmp_active_pages=0,tmp_total_pages=0, mmaped_size=0;
    uint64_t a1 = smap.from, a2 = smap.to;
    auto mapped_file = smap.mapped_file;

    uint64_t to_read = a2 - a1, read = 0;
    char buf[PAGE_SIZE * 3] = {};


    struct iovec local[1];
    local[0].iov_base = buf;
    local[0].iov_len = PAGE_SIZE;
    struct iovec remote[1];
    remote[0].iov_base = (void*)a1;
    remote[0].iov_len = PAGE_SIZE;

    bool present = is_page_present(f, a1);
    if (present) {
      process_vm_readv(pid, local, 1, remote, 1, 0);
    } else {
      memset(buf, 0, PAGE_SIZE);
    }
    bool seen_c_alloc = false;
    bool seen_rust_alloc = false;
    bool seen_mmap = false;
    bool seen_thread_create = false;
    uint64_t tmp_act_mmaped_size = 0, tmp_c_size = 0;


    uint64_t dealloc_c_pages_next = 0;


    // int cnt = 0;
    while (to_read > 0) {
      local[0].iov_base = buf + PAGE_SIZE;
      remote[0].iov_base = (void*)(a1 + read + PAGE_SIZE);
      bool next_present = is_page_present(f, a1 + read + PAGE_SIZE);
      if (next_present) {
        process_vm_readv(pid, local, 1, remote, 1, 0);
      } else {
        memset(buf + PAGE_SIZE, 0, PAGE_SIZE);
      }

      uint64_t mmap_func = 0;
      for (auto m:mmap) {
        if (m.first <= a1 + read && a1 + read < m.second.to) {
          mmap_func = m.second.func;
          break;
        }
      }

      uint64_t dealloc_c_offset = 0;
      for(int i =0;i<PAGE_SIZE;i+=1) {
        //if (mapped_file.size() > 0 && mapped_file[0] == '/') break;

        uint64_t magic = *(uint64_t*)&buf[i];
        if (magic == MAGIC || magic == MAGIC + 0x100) {

          uint64_t size = *(uint64_t*)&buf[i + sizeof(uint64_t)];
          uint64_t tid = *(uint64_t*)&buf[i + 2*sizeof(uint64_t)];
          uint64_t func = *(uint64_t*)&buf[i + 3*sizeof(uint64_t)];

          if (target_tid != 0 && target_tid != tid) {
            continue;
          }

          if (size > to_read || size <= 0) continue;
          auto real_s = calc_size(f, a1 + read + i + 32, size);

          auto diff = min((uint64_t)i, dealloc_c_offset + dealloc_c_pages_next) - dealloc_c_offset;
          if (diff && dealloc_func) func2size_c_dealloc[func].sum += diff;
          dealloc_c += diff;
          dealloc_c_pages_next = 0;
          dealloc_c_offset = 0;


          if (magic == MAGIC) {
            ptr2bi[a1 + read + i + 32] = {size, func, true, false};

            tid2size_c[tid].sum += real_s;
            tid2size_c[tid].cnt += 1;
            func2size_c[func].sum += real_s;
            func2size_c[func].cnt += 1;

            func2size_c[func].samples.push_back(a1 + read + i + 32);

            if (size < ALLOC_LIMIT && func > 2) {
                func2size_c_small[func].sum += size * 100;
                func2size_c_small[func].cnt += 1 * 100;
            }

            tracked_memory += size;
            seen_c_alloc = true;
            //}
            //
            if (mmap_func) overlap_c_mmap += real_s + 32;
            tmp_c_size += real_s + 32;


          }
          else {
            //ptr2bi[a1 + read + i + 32] = {size, func, true, true};

            seen_c_alloc = true;
            // dealloc_c += PAGE_SIZE - i;

            dealloc_c_pages_next = real_s + 32;
            dealloc_c_offset = i;

            // func2size_c_dealloc[func].sum += real_s;
            func2size_c_dealloc[func].cnt += 1;
            dealloc_func = func;
          }
        }

        if ((magic | 0xff) == (MAGIC_RUST | 0xff) || (magic | 0xff) == (MAGIC_RUST | 0xff) + 0x100) {
          int header_size = 24 + 8 * (magic & 0xff);

          uint64_t size = *(uint64_t*)&buf[i + sizeof(uint64_t)];
          uint64_t tid = *(int64_t*)&buf[i + 2*sizeof(uint64_t)];
          uint64_t func = *(uint64_t*)&buf[i + 3*sizeof(uint64_t)];
          if (size > to_read || size <= 0) continue;
          // if (size <= 0 || size > read + PAGE_SIZE) continue;
          auto real_s = calc_size(f, a1 + read + i + header_size, size);

          if (target_tid != 0 && target_tid != tid) {
            continue;
          }

          if ((magic | 0xff) == (MAGIC_RUST | 0xff)) {
            ptr2bi[a1 + read + i + header_size] = {size, func, false, false};

            func2size_rust[func].sum += real_s;
            func2size_rust[func].cnt += 1;
            tracked_memory += size;
            seen_rust_alloc = true;

            func2size_rust[func].samples.push_back(a1 + read + i + header_size);
            if (size < ALLOC_LIMIT && func > 2) {
                func2size_rust_small[func].sum += size * 100;
                func2size_rust_small[func].cnt += 1 * 100;
            }

            if (func == 2) {
              seen_thread_create = true;
            }
          } else {
            if (func < 0x700000000000) {
              seen_rust_alloc = true;
              dealloc_rust += real_s + header_size;
              func2size_rust_dealloc[func].sum += real_s;
              func2size_rust_dealloc[func].cnt += 1;
            }
          }

        }
      }
      if (dealloc_c_pages_next > 0) {
        uint64_t diff = PAGE_SIZE - dealloc_c_offset;
        uint64_t decrease = min(diff, (uint64_t)dealloc_c_pages_next);
        if (decrease && dealloc_func) func2size_c_dealloc[dealloc_func].sum += decrease;
        dealloc_c += decrease;
        dealloc_c_pages_next -= decrease;

      }
      {
        if (mmap_func) {
          seen_mmap = true;

          if (present) {
            mmap_func2size[mmap_func].sum += PAGE_SIZE;
            mmap_func2size[mmap_func].cnt += 1;
            tmp_act_mmaped_size += PAGE_SIZE;
            if (mapped_file.size() >= 1 && mapped_file[0] == '/') {
              mmaped_file_size += PAGE_SIZE;
            }
          }
          mmap_func2size2[mmap_func].sum += PAGE_SIZE;
          mmap_func2size2[mmap_func].cnt += 1;
          mmaped_size += PAGE_SIZE;
        } else {
          if (present) {
            if (mapped_file.size() >= 1 && mapped_file[0] == '/') {
              not_mmaped_file_size += PAGE_SIZE;
            }
          }
        }
      }

      if (present) {
        active_pages++;
        tmp_active_pages++;
      }
      pages++;
      tmp_total_pages++;

      to_read -= PAGE_SIZE;
      read += PAGE_SIZE;

      memcpy(buf, buf + PAGE_SIZE, PAGE_SIZE);

      present = next_present;
    }
    if (seen_c_alloc) c_active_pages += tmp_active_pages;

    if (!(mapped_file.size() >= 1 && mapped_file[0] == '/')) {
      unknown_mem += PAGE_SIZE * tmp_active_pages - tmp_act_mmaped_size - tmp_c_size;
      if (seen_c_alloc) {
        unknown_mem_possibly_c += PAGE_SIZE * tmp_active_pages - tmp_act_mmaped_size - tmp_c_size;
      }
    }
// #ifdef DEBUG
    printf( "addr: %" PRIx64 "-%" PRIx64 " pages: %ld active_pages: %ld mmaped: %.2lfMiB = %s flags %s %s %s %s\n",
        a1, a2, tmp_total_pages, tmp_active_pages, mmaped_size / MiB, mapped_file.c_str(), seen_c_alloc ? "C" : "", seen_rust_alloc ? "RUST": "",
        seen_mmap ? "MMAP": "",
        seen_thread_create ? "THREAD_CREATE" : ""
        );
// #endif
  }
  printf("pages: %.2lfMiB active_pages: %.2lfMiB tracked_memory: %.2lfMiB file_size: %.2lfMiB\n", pages*PAGE_SIZE / MiB, active_pages*PAGE_SIZE / MiB, tracked_memory / MiB, (not_mmaped_file_size + mmaped_file_size) / MiB);

  ptr2bi[0xffffffffffffffff] = {0,0,false};

  for (auto smap : smaps) {
    // uint64_t tmp_active_pages=0,tmp_total_pages=0, mmaped_size=0;
    uint64_t a1 = smap.from, a2 = smap.to;
    auto mapped_file = smap.mapped_file;

    uint64_t to_read = a2 - a1, read = 0;
    char buf[PAGE_SIZE * 3] = {};


    struct iovec local[1];
    local[0].iov_base = buf;
    local[0].iov_len = PAGE_SIZE;
    struct iovec remote[1];
    remote[0].iov_base = (void*)a1;
    remote[0].iov_len = PAGE_SIZE;

    bool present = is_page_present(f, a1);
    if (present) {
      process_vm_readv(pid, local, 1, remote, 1, 0);
    } else {
      memset(buf, 0, PAGE_SIZE);
    }

    uint64_t cur_ptr = 0;
    uint64_t cur_func = 0;
    uint64_t cur_size = 0;

    uint64_t rust_next_ptr = 0;
    uint64_t rust_next_size = 0;
    uint64_t tracked_ptr = 0, untracked_ptr = 0;

    while (to_read > 0) {

      local[0].iov_base = buf + PAGE_SIZE;
      remote[0].iov_base = (void*)(a1 + read + PAGE_SIZE);
      bool next_present = is_page_present(f, a1 + read + PAGE_SIZE);
      if (next_present) {
        process_vm_readv(pid, local, 1, remote, 1, 0);
      } else {
        memset(buf + PAGE_SIZE, 0, PAGE_SIZE);
      }


      // uint64_t dealloc_c_offset = 0;
      for(int i =0;i<PAGE_SIZE;i+=1) {
        //if (mapped_file.size() > 0 && mapped_file[0] == '/') break;
        if (a1 + read + i >= rust_next_ptr + rust_next_size) {
          auto tmp = ptr2bi.lower_bound(a1 + read + i);
          rust_next_ptr = tmp->first;
          rust_next_size = tmp->second.size;
          // rust_next_func = tmp->second.func;
        }
        uint64_t maybe_ptr = *(uint64_t*)&buf[i];
        uint64_t magic = *(uint64_t*)&buf[i];
        if ((magic | 0xff) == (MAGIC_RUST | 0xff) || (magic | 0xff) == (MAGIC | 0xff)) {
          uint64_t size = *(uint64_t*)&buf[i + sizeof(uint64_t)];
          uint64_t func = *(uint64_t*)&buf[i + 3*sizeof(uint64_t)];

          const int header_size = 24 + 8 * (magic & 0xff);

          if (func) {
            cur_ptr = a1 + read + i + header_size;
            cur_func = func;
            cur_size = size + header_size;
          }
        }
        if ((maybe_ptr >= 0x550000000000 && maybe_ptr <= 0x560000000000)
         || (maybe_ptr >= 0x7f0000000000 && maybe_ptr <= 0x7fffffffffff)) {
          if (ptr2bi.count(maybe_ptr)) {


            if (cur_size >= 1) {
                ptr2bi[maybe_ptr].prev.push_back(cur_ptr);
              tracked_ptr++;
              if (cur_func > 2 && cur_func == ptr2bi[maybe_ptr].func) {

                //ptr2bi[maybe_ptr].ref+=1000000;
              }
              ptr2bi[maybe_ptr].ref+=1;
            }
            else {
              ptr2bi[maybe_ptr].ref++;

              untracked_ptr++;
            }
          }
        }
        if (cur_size) cur_size -= 1;
      }

      to_read -= PAGE_SIZE;
      read += PAGE_SIZE;

      memcpy(buf, buf + PAGE_SIZE, PAGE_SIZE);

      present = next_present;
    }
    printf( "addr: %" PRIx64 "-%" PRIx64 " untracked: %lu/%lu \n", a1, a2, tracked_ptr, untracked_ptr);
  }

  uint64_t sum=0,cnt=0;
  for(auto elem: tid2size_c) { printf("tid(C) %ld: %.2lfMiB/%ld\n", elem.first, elem.second.sum / MiB, elem.second.cnt); }



  auto process_func = [&](map<uint64_t, Counter> &arg, string pat, map<string, Counter> &acum, bool do_acum, bool wasm_stats, bool check_prev_calls) {

    uint64_t other_sum = 0, other_cnt = 0;

    for(auto elem: arg) {
      string file = "<UNKNOWN>";
      for (auto entry : symbols) {
        if (entry.from <= elem.first && elem.first < entry.to) file = entry.file;
      }
      auto file2 = find_file2(elem.first);
      if (elem.first == 1) { file = file2 = "<SMALL_ALLOC_100B>"; }
      if (elem.first == 2) { file = file2 = "<THREAD_CREATE>"; }

      auto file2_lower = tolower(file2);

      if (wasm_stats) {
        if (file2_lower.find("wasm") != string::npos) { wasmer_size += elem.second.sum, wasmer_cnt += elem.second.cnt; }
        if (file2_lower.find("rocksdb") != string::npos) { rocksdb_size += elem.second.sum, rocksdb_cnt += elem.second.cnt; }
        if (file2_lower.find("backtrace") != string::npos) { libbacktrace_size += elem.second.sum, libbacktrace_cnt += elem.second.cnt; }
        if (file.find("chain/") != string::npos
            || file.find("core/") != string::npos
            || file.find("neard/") != string::npos
            || file.find("runtime/") != string::npos) {
          near_size += elem.second.sum, near_cnt += elem.second.cnt;
        }
      }

      if (do_acum) {
        acum[file].sum += elem.second.sum;
        acum[file].cnt += elem.second.cnt;
      }
      if (elem.second.sum < min_printable_size) {
        other_sum += elem.second.sum;
        other_cnt += elem.second.cnt;

      } else {
        printf("%s %" PRIx64 " %" PRIx64 " %s: %.2lfMiB/%ld\n", pat.c_str(), get_func_bin_ptr(elem.first, smaps_cache), elem.first, file2.c_str(), elem.second.sum / MiB, elem.second.cnt);

        if (check_prev_calls) cout << elem.second.samples.size() << endl;

        if (file2_lower.find("rocksdb") != string::npos
        ||  file2_lower.find("wasm") != string::npos
        || true) {
          bool found = 0;
          uint64_t ref = 0;
          vector<ResultEntry> res;
          int64_t res_score = 0;
          size_t tries = 0;

          auto start = time(NULL);
          if (COMPUTE_TRACES) {
            for (auto ptr : elem.second.samples) {
              ref += ptr2bi[ptr].ref;
              if (ptr2bi[ptr].prev.size() == 0) continue;
              found = 1;

              set<long long> seen;
              set<pair<long long, string>> seen2;
              vector<ResultEntry> tmp_res;
              set<string> tmp_res_score;

              int cnt = 100;

              queue<pair<int, uint64_t>> q;
              q.push({0, ptr});

  // #define RECOVER_STACK_NEW

#ifdef RECOVER_STACK_NEW
              while(!q.empty()) {
                auto ptr_pair = q.front();
                auto ptr= ptr_pair.second;
                q.pop();
                auto e = ptr2bi[ptr];
                auto file3 = find_file2(ptr2bi[ptr].func);
                auto size = ptr2bi[ptr].size;
                if (!seen2.count({size, file3})) {
                  if (file3.find("actix") != string::npos) continue;
                  if (file3.find("tokio") != string::npos) continue;
                  seen2.insert({size, file3});
                  tmp_res.push_back({ptr_pair.first, ptr,  e.size, file3, ptr2bi[ptr].func});
                }
                for (auto ptr2: e.prev) {
                  if (seen.count(ptr2)) continue;
                  seen.insert(ptr2);

                  q.push({ptr_pair.first + 1, ptr2});
                }
              }
#else
              for (int cnt2=0;cnt--;cnt2++) {
                auto e = ptr2bi[ptr];
                seen.insert(ptr);
                auto file3 = find_file2(e.func);
                tmp_res.push_back({cnt2, ptr,  e.size, file3, e.func});
                if (file3.size() > 0 && file3[0] != '<') tmp_res_score.insert(file3);

                if (e.prev.size() == 0) break;
                bool f = 0;

                for (auto ptr2: e.prev) {
                  if (seen.count(ptr2)) continue;
                  seen.insert(ptr2);
                  ptr = ptr2;
                  auto file3 = find_file2(ptr2bi[ptr].func);

                  if (file3.find("backtrace") != string::npos) continue;
                  if (file3.find("tokio::runtime::enter::ENTERED") != string::npos) {
                    //cout<<hex<<" "<<e.func<<" ENTERED"<<endl;
                    continue;
                  }
                  f = 1;

                  break;
                }
                if (f == 0) break;
              }
#endif
              if (tmp_res.size() <= 1) continue;
              if (tmp_res.size() == 2 && tmp_res.back().path.find("backtrace") != string::npos) continue;
              if (tmp_res.size() == 2 && tmp_res.back().path.find("tokio::runtime::enter::ENTERED") != string::npos) continue;

              int64_t new_score = tmp_res_score.size() * 1000 - tmp_res.size();

              if (res_score <= new_score) {
                res_score = new_score;
                res = tmp_res;
              }
              if (tries++ >= 10000) break;
              if (time(NULL) - start > 15) break;

              // break;
            }
          }
          for(auto r:res) cout<<r.depth<<" "<<hex<<r.ptr<<" "<<r.func<<" "<<dec<<r.size<<" "<<arg[r.func].cnt<<" "<<r.path<<endl;
          cout<<"found "<<found<<" ref "<<ref<<endl;
        }
      }
    }
    printf("%s %" PRIx64 " %s: %.2lfMiB/%ld\n", pat.c_str(), (uint64_t)0, "(OTHER)", other_sum / MiB, other_cnt);
  };

  process_func(func2size_c, "(C) func", file2size_c, true, true, true);
  process_func(func2size_c_small, "(C small) func", file2size_c, false, true, false);
  process_func(func2size_c_dealloc, "(C dealloc) func", file2size_c, false, false, false);

  process_func(func2size_rust, "(Rust) func", file2size_rust, true, true, false);
  process_func(func2size_rust_small, "(Rust small) func", file2size_rust, false, true, false);
  process_func(func2size_rust_dealloc, "(Rust dealloc) func", file2size_rust, false, false, false);


  sum=0;cnt=0;
  for(auto elem: file2size_c) {
    if (elem.second.sum >= min_printable_size) {
      //printf("file %s: %.2lfMiB/%ld\n", elem.first.c_str(), elem.second.sum / MiB, elem.second.cnt);
    }
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  printf("(C code)sum=%.2lfMiB sum(with proxy overhead)=%.2lfMiB cnt=%ld\n", sum / MiB, (sum + cnt*32)/MiB, cnt);
  uint64_t c_over = (sum + cnt*32);
  allocator_headers += cnt * 32;
  sum=0;cnt=0;

  for(auto elem: file2size_rust) {
    //if (elem.second.sum > minimum_printable_size)
    //printf("(rust)file %s: %.2lfMiB/%ld\n", elem.first.c_str(), elem.second.sum / MiB, elem.second.cnt);
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  uint64_t rust_over = (sum + cnt*32);
  allocator_headers += cnt * 32;
  printf("(Rust code)sum=%.2lfMiB sum(with proxy overhead)=%.2lfMib cnt=%ld\n", sum / MiB, (sum + cnt*32)/MiB, cnt);

  sum=0;cnt=0;
  for(auto elem: func2size_rust_small) {
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  printf("(Rust small code)sum=%.2lfMiB sum(with proxy overhead)=%.2lfMib cnt=%ld\n", sum / MiB, (sum + cnt*32)/MiB, cnt);

  sum=0;cnt=0;
  for(auto elem: mmap_func2size) {
    string file = "<UNKNOWN>", file2;
    // uint64_t dist = 0x7fffffff;
    for (auto entry : symbols) {
      if (entry.from <= elem.first && elem.first < entry.to) file = entry.file;
    }
    file2 = find_file2(elem.first);
    mmap_file2size[file].sum += elem.second.sum;
    mmap_file2size[file].cnt += elem.second.cnt;
    if (file2.find("wasm") != string::npos) wasmer_size += elem.second.sum;
    if (file2.find("rocksdb") != string::npos) rocksdb_size += elem.second.sum;

    printf("file(resident mmap) %s %s %p: %.2lfMiB/%ld\n", file.c_str(), file2.c_str(), (void*)elem.first, elem.second.sum / MiB, elem.second.cnt);
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  uint64_t mmaped_act = sum;
  printf("sum=%.2lfMiB cnt=%ld\n", sum / MiB, cnt);
  sum=0;cnt=0;
  for(auto elem: mmap_func2size2) {
    string file = "<UNKNOWN>", file2;
    // uint64_t dist = 0x7fffffff;
    for (auto entry : symbols) {
      if (entry.from <= elem.first && elem.first < entry.to) file = entry.file;
    }
    file2 = find_file2(elem.first);

    mmap_file2size2[file].sum += elem.second.sum;
    mmap_file2size2[file].cnt += elem.second.cnt;
    printf("file(mmap) %s %s %p: %.2lfMiB/%ld\n", file.c_str(), file2.c_str(), (void*)elem.first, elem.second.sum / MiB, elem.second.cnt);
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  printf("sum=%.2lfMiB cnt=%ld\n", sum / MiB, cnt);

  /*
  sum=0;cnt=0;
  for(auto elem: mmap_file2size) {
    //printf("(mmap resident)file %s: %.2lfMiB/%ld\n", elem.first.c_str(), elem.second.sum / MiB, elem.second.cnt);
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  printf("sum=%.2lfMiB cnt=%ld\n", sum / MiB, cnt);
  */

#ifdef DEBUG
  sum=0;cnt=0;
  for(auto elem: mmap_file2size2) {
    printf("(mmap total)file %s: %.2lfMiB/%ld\n", elem.first.c_str(), elem.second.sum / MiB, elem.second.cnt);
    sum += elem.second.sum, cnt += elem.second.cnt;
  }
  printf("sum=%.2lfMiB cnt=%ld\n", sum / MiB, cnt);
#endif

  printf("\n");
  printf("STATS:\n");
  printf("RSS %.2lfMiB = C + MAPPED + NOT_MMAPED_FILE_SIZE - OVERLAP_C_MMAP + UNKNOWN_MEM_NOT_C + UNKNOWN_MEM_C\n", active_pages * PAGE_SIZE / MiB);
  printf("C %.2lfMiB\n", c_over / MiB);
  printf("C_DEALLOCATED %.2lfMiB\n", dealloc_c / MiB);
  printf("RUST %.2lfMiB\n", rust_over / MiB);
  printf("RUST_DEALLOCATED %.2lfMiB\n", dealloc_rust / MiB);
  printf("MMAPED %.2lfMiB = RUST + BACKTRACE + ...\n", mmaped_act / MiB);
  printf("NOT_MMAPED_FILE_SIZE %.2lfMiB\n", not_mmaped_file_size / MiB);
  printf("MMAPED_FILE_SIZE %.2lfMiB\n", mmaped_file_size / MiB);
  printf("OVERLAP_C_MMAP %.2lfMiB\n", overlap_c_mmap / MiB);
  printf("UNKNOWN_MEM_NOT_C %.2lfMiB\n", (unknown_mem - unknown_mem_possibly_c) / MiB);
  printf("UNKNOWN_MEM_C %.2lfMiB\n", unknown_mem_possibly_c / MiB);
  printf("\n");
  printf("WASMER %.2lfMiB %ld alloc\n", wasmer_size / MiB, wasmer_cnt);
  printf("ROCKSDB %.2lfMiB %ld alloc\n", rocksdb_size / MiB, rocksdb_cnt);
  printf("NEARCORE %.2lfMiB %ld alloc\n", near_size / MiB, near_cnt);
  // printf("LOADED_FILE_SIZE %.2lfMiB\n", (not_mmaped_file_size + mmaped_file_size) / MiB);
  printf("RESIDENT_MEM_NOT_USED_BY_ALLOCATOR_C %.2lfMiB\n", (dealloc_c) / MiB);
  printf("RESIDENT_MEM_NOT_USED_BY_ALLOCATOR_RUST %.2lfMiB\n", (dealloc_rust) / MiB);
  printf("LIBBACKTRACE %.2lfMiB %ld alloc (can be easily reduced to 1mb)\n", libbacktrace_size / MiB, libbacktrace_cnt);
  printf("ALLOCATOR_HEADER_OVERHEAD_FOR_DEBUGGING %.2lfMiB (can be easily cut in half)\n", allocator_headers / MiB);


  return 0;
}
