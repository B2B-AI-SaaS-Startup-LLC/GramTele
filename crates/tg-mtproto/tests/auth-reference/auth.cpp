#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QFile>
#include <cstdio>
int main(int argc,char**argv){
 if(argc!=3)return 1;QFile input(QString::fromUtf8(argv[1]));if(!input.open(QIODevice::ReadOnly))return 2;auto args=QJsonDocument::fromJson(input.readAll()).object();auto phone=args["phone"].toString();auto signPhone=args.contains("signin_phone")?args["signin_phone"].toString():phone;auto hash=args["hash"].toString();auto code=args["code"].toString();
 const auto send=MTPauth_SendCode(MTP_string(phone),MTP_int(17349),MTP_string("344583e45741c457fe1862106095a5eb"),MTP_codeSettings(MTP_flags(0),MTPVector<MTPbytes>(),MTPstring(),MTPBool()));
 const auto signin=MTPauth_SignIn(MTP_flags(MTPauth_SignIn::Flag::f_phone_code),MTP_string(signPhone),MTP_string(hash),MTP_string(code),MTP_emailVerificationCode(MTP_string("")));
 FILE*out=std::fopen(argv[2],"wb");if(!out)return 3;auto write=[&](const auto&v){mtpBuffer b;v.write(b);uint32_t n=b.size()*4;std::fwrite(&n,4,1,out);std::fwrite(b.data(),1,n,out);};write(send);write(signin);
 auto params=MTP_jsonObject(MTP_vector<MTPJSONObjectValue>(QVector<MTPJSONObjectValue>{MTP_jsonObjectValue(MTP_string("tz_offset"),MTP_jsonNumber(MTP_double(0)))}));
 auto init=MTPInitConnection<MTPauth_SendCode>(MTP_flags(MTPInitConnection<MTPauth_SendCode>::Flag::f_params),MTP_int(17349),MTP_string("Desktop"),MTP_string("Linux"),MTP_string("7.2.9"),MTP_string("en"),MTP_string("tdesktop"),MTP_string("en"),MTPInputClientProxy(),params,send);
 write(MTPInvokeWithLayer<decltype(init)>(MTP_int(229),init));std::fclose(out);return 0;
}
