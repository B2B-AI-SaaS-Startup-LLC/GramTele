// Record the implementation-defined Linux libstdc++ unordered_set bucket policy.
#include <unordered_set>
#include <iostream>
int main(){std::__detail::_Prime_rehash_policy p;size_t n=0;std::cout<<"[";for(;;){size_t b=p._M_next_bkt(n);std::cout<<b;if(b==n-1 || b>=(size_t(1)<<63))break;std::cout<<",";n=b+1;}std::cout<<"]\n";}
